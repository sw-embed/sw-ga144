use gloo_timers::callback::Interval;
use sw_ga144::{COLUMNS, Ga144, Image, NodeId, ROWS};
use web_sys::{HtmlSelectElement, HtmlTextAreaElement};
use yew::prelude::*;

mod demos;
mod distributed;
mod topology;

use demos::{Demo, MULTIPLE_NODES, SINGLE_NODE};
use distributed::{Distributed, SumCallout, sum_node};
use topology::{Network, Topology};

struct Program {
    entry: u16,
    listing: String,
    source: Option<&'static str>,
}

struct Machine {
    chip: Ga144,
    ticks: usize,
    status: String,
    selected: NodeId,
    programs: Vec<(NodeId, Program)>,
}

impl Machine {
    fn empty() -> Self {
        Self {
            chip: Ga144::new(),
            ticks: 0,
            status: "Ready".into(),
            selected: NodeId::from_number(0).expect("valid first node"),
            programs: Vec::new(),
        }
    }

    fn new(demo: &Demo) -> Self {
        let mut machine = Self::empty();
        machine.selected = NodeId::from_number(demo.result_node).expect("valid demo result node");
        for program in demo.programs {
            let node = NodeId::from_number(program.node).expect("valid demo node");
            machine
                .install(node, program.entry, program.listing, Some(program.source))
                .expect("valid assembled demo");
        }
        machine
    }

    fn install(
        &mut self,
        node: NodeId,
        entry: u16,
        listing: &str,
        source: Option<&'static str>,
    ) -> Result<(), String> {
        let image = listing
            .parse::<Image>()
            .map_err(|error| error.to_string())?;
        self.chip
            .start(node, entry)
            .map_err(|error| error.to_string())?;
        self.chip.load_ram(node, &image);
        self.programs.retain(|(id, _)| *id != node);
        self.programs.push((
            node,
            Program {
                entry,
                listing: listing.into(),
                source,
            },
        ));
        Ok(())
    }

    fn program(&self, node: NodeId) -> Option<&Program> {
        self.programs
            .iter()
            .find(|(id, _)| *id == node)
            .map(|(_, program)| program)
    }

    fn step(&mut self, count: usize) {
        for _ in 0..count {
            self.ticks += 1;
            match self.chip.tick() {
                Ok(0) => {
                    self.status = "Waiting for a port transfer".into();
                    return;
                }
                Ok(_) => self.status = "Advanced".into(),
                Err(error) => {
                    self.status = error.to_string();
                    return;
                }
            }
        }
    }
}

#[function_component(App)]
fn app() -> Html {
    let selection = use_state(|| 0);
    let topology = use_state(|| Topology::Single);
    let chip_selection = use_state(|| 0);
    let single = *topology == Topology::Single;
    let help_open = use_state(|| false);
    let toggle_help = {
        let help_open = help_open.clone();
        Callback::from(move |_| help_open.set(!*help_open))
    };
    let distributed_option = demos::all().count();
    let sum_mode = !single || *selection == distributed_option;
    let demo = demos::all().nth(*selection).unwrap_or(&SINGLE_NODE[0]);
    let single_machine = use_mut_ref(|| Machine::new(&SINGLE_NODE[0]));
    let distribution = use_mut_ref(|| Distributed::new(Topology::Single));
    let running = use_state(|| false);
    let machine = if sum_mode {
        distribution.borrow().chip(*chip_selection)
    } else {
        single_machine.clone()
    };
    let redraw = use_state(|| 0);
    let editor = use_node_ref();
    let entry_field = use_node_ref();

    let select_chip = {
        let chip_selection = chip_selection.clone();
        let running = running.clone();
        Callback::from(move |index| {
            running.set(false);
            chip_selection.set(index);
        })
    };
    let reset_sum = {
        let chip_selection = chip_selection.clone();
        let distribution = distribution.clone();
        let running = running.clone();
        Callback::from(move |layout| {
            running.set(false);
            *distribution.borrow_mut() = Distributed::new(layout);
            chip_selection.set(0);
        })
    };
    let change_demo = {
        let redraw = redraw.clone();
        let selection = selection.clone();
        let reset_sum = reset_sum.clone();
        Callback::from(move |event: Event| {
            let select = event.target_unchecked_into::<HtmlSelectElement>();
            let Ok(index) = select.value().parse::<usize>() else {
                return;
            };
            if index == distributed_option {
                reset_sum.emit(Topology::Single);
            } else if let Some(demo) = demos::all().nth(index) {
                *single_machine.borrow_mut() = Machine::new(demo);
                reset_sum.emit(Topology::Single);
            } else {
                return;
            }
            selection.set(index);
            redraw.set(*redraw + 1);
        })
    };
    let reset = {
        let machine = machine.clone();
        let redraw = redraw.clone();
        let reset_sum = reset_sum.clone();
        let topology = *topology;
        Callback::from(move |_| {
            if sum_mode {
                reset_sum.emit(topology);
            } else {
                *machine.borrow_mut() = Machine::new(demo);
                redraw.set(*redraw + 1);
            }
        })
    };
    let advance_sum = {
        let running = running.clone();
        let distribution = distribution.clone();
        let chip_selection = chip_selection.clone();
        let redraw = redraw.clone();
        Callback::from(move |()| {
            let mut model = distribution.borrow_mut();
            if let Some(index) = model.step() {
                chip_selection.set(index);
            }
            if model.finished() {
                running.set(false);
            }
            redraw.set(*redraw + 1);
        })
    };
    {
        let advance_sum = advance_sum.clone();
        use_effect_with(*running, move |is_running| {
            let timer = is_running.then(|| Interval::new(320, move || advance_sum.emit(())));
            move || drop(timer)
        });
    }
    let step_sum = {
        let running = running.clone();
        Callback::from(move |_| {
            running.set(false);
            advance_sum.emit(());
        })
    };
    let run_sum = {
        let running = running.clone();
        Callback::from(move |_| running.set(!*running))
    };
    let advance = |count| {
        let machine = machine.clone();
        let redraw = redraw.clone();
        Callback::from(move |_| {
            machine.borrow_mut().step(count);
            redraw.set(*redraw + 1);
        })
    };
    let load = {
        let machine = machine.clone();
        let redraw = redraw.clone();
        let editor = editor.clone();
        let entry_field = entry_field.clone();
        Callback::from(move |_| {
            let text = editor.cast::<HtmlTextAreaElement>().unwrap().value();
            let entry = entry_field
                .cast::<web_sys::HtmlInputElement>()
                .unwrap()
                .value();
            let mut machine = machine.borrow_mut();
            match entry.parse::<u16>() {
                Ok(entry) if entry <= 1023 => {
                    let selected = machine.selected;
                    machine.status = match machine.install(selected, entry, &text, None) {
                        Ok(()) => format!("Loaded node {selected}"),
                        Err(error) => error,
                    };
                }
                _ => {
                    machine.status = "Entry address must be a decimal number from 0 to 1023".into()
                }
            }
            redraw.set(*redraw + 1);
        })
    };

    let model = distribution.borrow();
    let operation = model.last.clone();
    let finished = model.finished();
    let sum_controls = html! {
        <>
            <button onclick={reset.clone()}>{"Reset"}</button>
            <button onclick={step_sum} disabled={finished}>{"Step"}</button>
            <button class="primary" onclick={run_sum} disabled={finished}>{if *running { "Pause" } else { "Run demo" }}</button>
        </>
    };
    let callout = html! { <SumCallout step={model.steps} total={model.total_steps()}
    operation={operation.clone()} error={model.error.clone()}/> };
    let state = machine.borrow();
    let selected = state.selected;
    let registers = state.chip.inspect(selected);
    let node_grid = (0..ROWS).rev().flat_map(|row| (0..COLUMNS).map(move |col| (row, col))).map(|(row, col)| {
        let id = NodeId::new(row, col).unwrap();
        let machine = machine.clone();
        let redraw = redraw.clone();
        let status = state.chip.inspect(id);
        let mode = if status.waiting_address.is_some() { "waiting" } else if status.active { "active" } else { "idle" };
        let processing = sum_mode && id == sum_node() && operation.as_ref().is_some_and(|current| current.chip == *chip_selection);
        html! {
            <button type="button" class={classes!("node", mode, processing.then_some("processing"))} aria-pressed={(id == selected).to_string()}
                title={format!("Node {id} ({mode})")} onclick={Callback::from(move |_| {
                    machine.borrow_mut().selected = id;
                    redraw.set(*redraw + 1);
                })}>{id.to_string()}</button>
        }
    });
    let program = state.program(selected);
    let instructions = program.and_then(|program| program.source);
    let listing = program.map_or("", |program| &program.listing);
    let entry = program.map_or(0, |program| program.entry);
    let wait = registers
        .waiting_address
        .map_or_else(|| "—".into(), |address| format!("x{address:03X}"));
    html! {
        <main>
            <header>
                <div><h1>{"GA144 Studio"}</h1><p>{"Step arrayForth through a simulated 144-node chip."}</p></div>
                <div class="header-actions">
                    <button type="button" aria-expanded={help_open.to_string()} aria-controls="demo-help" onclick={toggle_help}>{"Help"}</button>
                </div>
            </header>
            <nav class="topology-tabs" aria-label="Chip topology">
                {for Topology::ALL.into_iter().map(|tab| {
                    let topology = topology.clone();
                    let reset_sum = reset_sum.clone();
                    html! {
                        <button type="button" aria-pressed={(tab == *topology).to_string()}
                            onclick={Callback::from(move |_| {
                                if *topology != tab {
                                    reset_sum.emit(tab);
                                    topology.set(tab);
                                }
                            })}>{tab.title()}</button>
                    }
                })}
            </nav>
            <section id="demo-help" class="panel help" hidden={!*help_open} aria-labelledby="help-heading">
                <h2 id="help-heading">{"How to use GA144 Studio"}</h2>
                <ol>
                    <li>{"Pick a demo. Studio loads its programs and selects the node where the answer will appear."}</li>
                    <li>{"Click Run demo to advance the chip, or Step to advance one tick. Reset reloads the selected demo. Click any square to inspect that node."}</li>
                    <li>{"Look at T for the demo result. T is the top of the node’s data stack. The inspector also shows its source, RAM, and other registers."}</li>
                    <li>{"For your own program, run "}<code>{"./assemble program.f18"}</code>{", choose a node, and paste the listing under Load an assembler listing. Use entry address 0 for a program that starts at RAM address 0."}</li>
                </ol>
                <h3>{"Distributed sum"}</h3>
                <p>{"Pick Distributed sum on Single, or open Dual, Ring, or Mesh to run it across 2, 5, or 62 chips. The mesh keeps Chip 63 and Chip 64 idle."}</p>
                <p>{"Step processes one chip. Run demo plays the steps in order, and Pause stops playback. The highlighted chip and Node 000 show where the current subtotal was calculated. The callout shows which values were added."}</p>
                <p>{"Node 000 on each chip runs F18 code. The browser passes the subtotals along the diagram’s links. This transport models who sends to whom and does not model serial timing or signaling."}</p>
                <p>{"The inspector displays results directly. You can also send values between simulated nodes."}</p>
                <p>{"This demo is conceptual and based on public documentation. It has not been validated against actual GreenArrays hardware or software."}</p>
            </section>
            {if !single { html! {
                <Network topology={*topology} selected={*chip_selection} on_select={select_chip}
                    operation={operation.clone()} controls={sum_controls.clone()}>{callout.clone()}</Network>
            } } else { Html::default() }}
            <section class="panel">
                <div class="panel-title">
                    <h2>{if single { "Chip map".into() } else { format!("Chip {:02} node map", *chip_selection + 1) }}</h2>
                    {if single { html! { <div class="controls">
                            <div class="demo-picker">
                                <label for="demo">{"Demo"}</label>
                                <select id="demo" onchange={change_demo} aria-describedby={(!sum_mode).then_some("demo-description")}>
                                    <optgroup label="Single node">
                                        {for SINGLE_NODE.iter().enumerate().map(|(index, demo)|
                                            html! { <option value={index.to_string()} selected={index == *selection}>{demo.title}</option> })}
                                    </optgroup>
                                    <optgroup label="Multiple nodes">
                                        {for MULTIPLE_NODES.iter().enumerate().map(|(index, demo)| {
                                            let index = index + SINGLE_NODE.len();
                                            html! { <option value={index.to_string()} selected={index == *selection}>{demo.title}</option> }
                                        })}
                                    </optgroup>
                                    <option value={distributed_option.to_string()} selected={sum_mode}>{"Distributed sum"}</option>
                                </select>
                            </div>
                        {if sum_mode { sum_controls.clone() } else { html! {
                            <>
                                <button onclick={reset}>{"Reset"}</button>
                                <button onclick={advance(1)}>{"Step"}</button>
                                <button class="primary" onclick={advance(demo.ticks)}>{"Run demo"}</button>
                            </>
                        } }}
                    </div> } } else { Html::default() }}
                </div>
                {if !sum_mode { html! { <p id="demo-description">{demo.description}</p> } }
                 else if single { callout } else { Html::default() }}
                <div class="scroll"><div class="chip">{for node_grid}</div></div>
                <div class="legend">
                    <span><i class="active"></i>{"Active"}</span><span><i class="waiting"></i>{"Waiting"}</span><span><i class="idle"></i>{"Idle"}</span>
                    {if !sum_mode { html! {
                        <output aria-live="polite">{format!("Tick {} · {}", state.ticks, state.status)}</output>
                    } } else { Html::default() }}
                </div>
            </section>
            <div class="lower">
                <section class="panel">
                    <div class="panel-title"><h2>{format!("Node {selected}")}</h2><span class="detail">{"Registers and RAM"}</span></div>
                    <dl class="registers">
                    {for [
                        ("T", registers.t.to_string()), ("S", registers.s.to_string()),
                        ("R", registers.r.to_string()), ("P", format!("x{:03X}", registers.p)),
                        ("A", format!("x{:05X}", registers.a)), ("B", format!("x{:03X}", registers.b)),
                        ("Slot", registers.slot.to_string()), ("Wait", wait),
                    ].into_iter().map(|(name, value)| html! {<div><dt>{name}</dt><dd>{value}</dd></div>})}
                    </dl>
                    <div class="ram" aria-label="RAM words">{ for state.chip.ram(selected).iter().enumerate().map(|(i, word)| html! {
                        <span title={format!("RAM address x{i:02X}")} class={classes!((word != &sw_ga144::ENCODING).then_some("written"))}>{format!("{word:05X}")}</span>
                    }) }</div>
                </section>
                <section class="panel">
                    <div class="panel-title"><h2>{"Program"}</h2>
                        {if program.is_some() {
                            html! {<span class="detail">{if instructions.is_some() { "F18 source" } else { "Assembler listing" }}</span>}
                        } else { Html::default() }}
                    </div>
                    {if let Some(source) = instructions { html! {<pre>{source}</pre>} }
                      else if program.is_some() { html! {<pre>{listing}</pre>} }
                      else { html! {<p>{"This node has no loaded program."}</p>} }}
                    {if !sum_mode { html! {<details key={format!("listing-{selected}")}><summary>{"Load an assembler listing"}</summary>
                        <label for="listing">{"Decimal address and word pairs"}</label>
                        <textarea id="listing" ref={editor} value={listing.to_owned()} rows="5" spellcheck="false"/>
                        <div class="controls loader">
                            <label for="entry">{"Entry address"}</label>
                            <input id="entry" type="number" min="0" max="1023" ref={entry_field} value={entry.to_string()}/>
                            <button onclick={load}>{format!("Load node {selected}")}</button>
                        </div>
                    </details>} } else { Html::default() }}
                </section>
            </div>
            <footer>
                <span>{include_str!("../../COPYRIGHT").trim()}</span>
                <a href="LICENSE">{"MIT License"}</a>
                <a href="https://github.com/sw-embed/sw-ga144">{"Repository"}</a>
                <a href="https://www.greenarraychips.com/home/documents/downindex.php">{"GreenArrays documentation"}</a>
                <span title="Build host, Git revision, and UTC timestamp">{env!("GA144_BUILD")}</span>
            </footer>
        </main>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
