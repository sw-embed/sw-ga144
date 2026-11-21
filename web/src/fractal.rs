use std::{cell::RefCell, rc::Rc};

use yew::prelude::*;

use crate::{Machine, worker_node};

const SOURCE: &str = include_str!("../../examples/fractal-row.f18");
const LISTING: &str = include_str!(concat!(env!("OUT_DIR"), "/fractal-row.img"));
const SIZE: usize = 32;
const FRAME_COUNT: usize = 12;
pub(crate) const WORKER_COUNT: usize = 4;
const ROWS_PER_STEP: usize = 4;
const LAUNCH_INTERVAL: usize = 2;
const PARK_ADDRESS: u16 = 0x17;
const OUTPUT: usize = 0x20;
const ROW_LIMIT: usize = 1024;

#[derive(Debug, PartialEq)]
struct Frame {
    index: usize,
    pixels: Vec<bool>,
}

impl Frame {
    fn new(index: usize) -> Self {
        Self {
            index,
            pixels: Vec::with_capacity(SIZE * SIZE),
        }
    }

    fn rows(&self) -> usize {
        self.pixels.len() / SIZE
    }

    fn complete(&self) -> bool {
        self.rows() == SIZE
    }

    fn rasterize(&mut self, machine: &mut Machine) -> Result<(), String> {
        let stride = 2048_u32 >> self.index;
        let half_width = (SIZE as u32 / 2) * stride;
        let x = 0x5500_u32.saturating_sub(half_width);
        let first_y = 0x2a00_u32.saturating_sub(half_width);
        for _ in 0..ROWS_PER_STEP.min(SIZE - self.rows()) {
            let y = first_y + self.rows() as u32 * stride;
            let row = rasterize_row(machine, x, y, stride)?;
            self.pixels.extend(row);
        }
        Ok(())
    }
}

fn rasterize_row(
    machine: &mut Machine,
    x: u32,
    y: u32,
    stride: u32,
) -> Result<[bool; SIZE], String> {
    let listing = format!("{LISTING}24 {x}\n25 {y}\n26 {stride}\n");
    let node = worker_node();
    machine.install(node, 0, &listing, Some(SOURCE))?;
    machine.selected = node;
    for _ in 0..ROW_LIMIT {
        machine.chip.tick().map_err(|error| error.to_string())?;
        if machine.chip.inspect(node).p == PARK_ADDRESS {
            return Ok(std::array::from_fn(|column| {
                machine.chip.ram(node)[OUTPUT + column] == 0
            }));
        }
    }
    Err("The F18 row program did not reach its park address".into())
}

struct Worker {
    machine: Rc<RefCell<Machine>>,
    frame: Option<Frame>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkerStage {
    Computing,
    Queued,
    Reading,
}

pub(crate) struct Fractal {
    workers: [Worker; WORKER_COUNT],
    idle: Rc<RefCell<Machine>>,
    launched: usize,
    delivered: usize,
    reading: Option<usize>,
    step: usize,
    displayed: Option<Rc<Frame>>,
    error: Option<String>,
}

impl Fractal {
    pub(crate) fn new() -> Self {
        Self {
            workers: std::array::from_fn(|_| Worker {
                machine: Rc::new(RefCell::new(Machine::empty())),
                frame: None,
            }),
            idle: Rc::new(RefCell::new(Machine::empty())),
            launched: 0,
            delivered: 0,
            reading: None,
            step: 0,
            displayed: None,
            error: None,
        }
    }

    pub(crate) fn chip(&self, index: usize) -> Rc<RefCell<Machine>> {
        self.workers
            .get(index)
            .map_or(&self.idle, |worker| &worker.machine)
            .clone()
    }

    pub(crate) fn finished(&self) -> bool {
        self.delivered == FRAME_COUNT || self.error.is_some()
    }

    pub(crate) fn stage(&self, index: usize) -> Option<WorkerStage> {
        let frame = self.workers.get(index)?.frame.as_ref()?;
        Some(if self.reading == Some(index) {
            WorkerStage::Reading
        } else if frame.complete() {
            WorkerStage::Queued
        } else {
            WorkerStage::Computing
        })
    }

    pub(crate) fn stages(&self) -> Vec<Option<WorkerStage>> {
        (0..WORKER_COUNT).map(|index| self.stage(index)).collect()
    }

    pub(crate) fn step(&mut self) -> Option<usize> {
        if self.finished() {
            return None;
        }
        self.deliver_frame();
        self.begin_readback();
        let launched = self.launch_frame();
        self.step += 1;
        let mut computing = None;
        for (index, worker) in self.workers.iter_mut().enumerate() {
            if let Some(frame) = worker.frame.as_mut().filter(|frame| !frame.complete()) {
                computing.get_or_insert(index);
                if let Err(error) = frame.rasterize(&mut worker.machine.borrow_mut()) {
                    self.error = Some(error);
                    return None;
                }
            }
        }
        self.reading.or(launched).or(computing)
    }

    fn deliver_frame(&mut self) {
        if let Some(index) = self.reading.take() {
            let frame = self.workers[index]
                .frame
                .take()
                .expect("reading worker has a frame");
            self.displayed = Some(Rc::new(frame));
            self.delivered += 1;
        }
    }

    fn begin_readback(&mut self) {
        self.reading = self.workers.iter().position(|worker| {
            worker
                .frame
                .as_ref()
                .is_some_and(|frame| frame.index == self.delivered && frame.complete())
        });
    }

    fn launch_frame(&mut self) -> Option<usize> {
        if self.launched == FRAME_COUNT || !self.step.is_multiple_of(LAUNCH_INTERVAL) {
            return None;
        }
        let index = self
            .workers
            .iter()
            .position(|worker| worker.frame.is_none())?;
        self.workers[index].frame = Some(Frame::new(self.launched));
        self.launched += 1;
        Some(index)
    }

    pub(crate) fn view(&self) -> Html {
        html! {
            <div class="fractal-demo" aria-label="Fractal frame pipeline">
                <FrameView frame={self.displayed.clone()}/>
                <div class="fractal-progress">
                    <p><strong>{format!("{} of {FRAME_COUNT} frames displayed", self.delivered)}</strong></p>
                    <p>{"Each chip computes a whole 32 × 32 frame. Readback displays completed frames in order."}</p>
                    <ul aria-label="Chip jobs">
                        {for self.workers.iter().enumerate().map(|(index, worker)| {
                            let description = worker.frame.as_ref().map_or_else(|| "Idle".to_owned(), |frame| {
                                let number = frame.index + 1;
                                match self.stage(index).expect("worker has a frame") {
                                    WorkerStage::Computing => format!("Frame {number} · {} of {SIZE} rows", frame.rows()),
                                    WorkerStage::Queued => format!("Frame {number} · ready"),
                                    WorkerStage::Reading => format!("Frame {number} · reading back"),
                                }
                            });
                            html! { <li><strong>{format!("Chip {:02}", index + 1)}</strong><span>{description}</span></li> }
                        })}
                    </ul>
                    {if let Some(error) = &self.error { html! { <p role="alert">{error}</p> } } else { Html::default() }}
                </div>
            </div>
        }
    }
}

#[derive(Properties, PartialEq)]
struct FrameProps {
    frame: Option<Rc<Frame>>,
}

#[function_component(FrameView)]
fn frame_view(props: &FrameProps) -> Html {
    html! {
        <figure class="fractal-frame">
            {if let Some(frame) = &props.frame {
                html! {
                    <>
                        <div class="fractal-pixels" role="img" aria-label={format!("Sierpiński fractal frame {}", frame.index + 1)}>
                            {for frame.pixels.iter().map(|&pixel| html! { <span class={pixel.then_some("inside")}/> })}
                        </div>
                        <figcaption>{format!("Frame {} · {}× zoom", frame.index + 1, 1 << frame.index)}</figcaption>
                    </>
                }
            } else { html! {
                <div class="fractal-pixels empty">{"Waiting for the first frame"}</div>
            } }}
        </figure>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f18_row_matches_the_sierpinski_rule() {
        let mut chip = Machine::empty();
        for (x, y, stride) in [(0, 0, 2048), (0, 3, 1), (21824, 10784, 4)] {
            let row = rasterize_row(&mut chip, x, y, stride).unwrap();
            assert_eq!(
                row,
                std::array::from_fn(|column| (x + column as u32 * stride) & y == 0)
            );
        }
    }

    #[test]
    fn frames_overlap_and_display_in_order() {
        let mut demo = Fractal::new();
        let mut overlap = false;
        let mut last = 0;
        while !demo.finished() {
            demo.step();
            let computing = demo
                .stages()
                .into_iter()
                .filter(|stage| *stage == Some(WorkerStage::Computing))
                .count();
            overlap |= computing > 1 && demo.reading.is_some();
            if demo.delivered != last {
                assert_eq!(demo.displayed.as_ref().unwrap().index, last);
                last += 1;
            }
        }
        assert!(overlap);
        assert!(demo.error.is_none());
        assert_eq!(last, FRAME_COUNT);
    }
}
