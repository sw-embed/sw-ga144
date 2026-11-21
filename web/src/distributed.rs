use std::{cell::RefCell, collections::VecDeque, fmt::Write, rc::Rc};

use sw_ga144::NodeId;
use yew::prelude::*;

use crate::{Machine, topology::Topology};

const SOURCE: &str = include_str!("../../examples/distributed-sum.f18");
const LISTING: &str = include_str!(concat!(env!("OUT_DIR"), "/distributed-sum.img"));
const OPERAND_START: usize = 0x20;
const SUM_TICKS: usize = 32;

pub(crate) fn sum_node() -> NodeId {
    NodeId::from_number(0).expect("valid sum node")
}

#[derive(Clone, PartialEq)]
pub(crate) struct Operation {
    pub(crate) chip: usize,
    pub(crate) incoming: Vec<(usize, u32)>,
    pub(crate) sum: u32,
}

pub(crate) struct Distributed {
    chips: Vec<Rc<RefCell<Machine>>>,
    children: Vec<Vec<usize>>,
    order: Vec<usize>,
    pub(crate) steps: usize,
    pub(crate) last: Option<Operation>,
    pub(crate) error: Option<String>,
}

impl Distributed {
    pub(crate) fn new(topology: Topology) -> Self {
        let layout = topology.layout();
        let participants = topology.participants();
        let mut adjacent = vec![Vec::new(); participants];
        for (a, b) in layout.links {
            if a < participants && b < participants {
                adjacent[a].push(b);
                adjacent[b].push(a);
            }
        }

        // Breadth-first discovery builds a tree rooted at Chip 01.
        // Reversing discovery order puts every child before its parent.
        let mut children = vec![Vec::new(); participants];
        let mut seen = vec![false; participants];
        let mut queue = VecDeque::from([0]);
        let mut order = Vec::with_capacity(participants);
        seen[0] = true;
        while let Some(chip) = queue.pop_front() {
            order.push(chip);
            for &peer in &adjacent[chip] {
                if !seen[peer] {
                    seen[peer] = true;
                    children[chip].push(peer);
                    queue.push_back(peer);
                }
            }
        }
        assert_eq!(order.len(), participants, "disconnected demo topology");
        order.reverse();

        Self {
            chips: (0..layout.chips.len())
                .map(|_| Rc::new(RefCell::new(Machine::empty())))
                .collect(),
            children,
            order,
            steps: 0,
            last: None,
            error: None,
        }
    }

    pub(crate) fn chip(&self, index: usize) -> Rc<RefCell<Machine>> {
        self.chips.get(index).unwrap_or(&self.chips[0]).clone()
    }

    pub(crate) fn finished(&self) -> bool {
        self.steps == self.order.len() || self.error.is_some()
    }

    pub(crate) fn total_steps(&self) -> usize {
        self.order.len()
    }

    pub(crate) fn step(&mut self) -> Option<usize> {
        if self.finished() {
            return None;
        }
        let index = self.order[self.steps];
        match self.add_on_chip(index) {
            Ok(operation) => {
                self.steps += 1;
                self.last = Some(operation);
                Some(index)
            }
            Err(error) => {
                self.error = Some(error);
                None
            }
        }
    }

    fn add_on_chip(&self, index: usize) -> Result<Operation, String> {
        let incoming: Vec<_> = self.children[index]
            .iter()
            .map(|&child| (child, self.chips[child].borrow().chip.inspect(sum_node()).t))
            .collect();

        let mut operands = [0; 5];
        operands[0] = (index + 1) as u32;
        for (operand, &(_, value)) in operands[1..].iter_mut().zip(&incoming) {
            *operand = value;
        }
        let mut listing = LISTING.to_owned();
        for (offset, operand) in operands.into_iter().enumerate() {
            writeln!(listing, "{} {operand}", OPERAND_START + offset).expect("write to String");
        }

        let mut chip = self.chips[index].borrow_mut();
        chip.install(sum_node(), 0, &listing, Some(SOURCE))?;
        chip.selected = sum_node();
        for _ in 0..SUM_TICKS {
            chip.chip.tick().map_err(|error| error.to_string())?;
        }
        let sum = chip.chip.inspect(sum_node()).t;
        Ok(Operation {
            chip: index,
            incoming,
            sum,
        })
    }
}

#[derive(Properties, PartialEq)]
pub(crate) struct CalloutProps {
    pub(crate) step: usize,
    pub(crate) total: usize,
    pub(crate) operation: Option<Operation>,
    pub(crate) error: Option<String>,
}

#[function_component(SumCallout)]
pub(crate) fn callout(props: &CalloutProps) -> Html {
    let finished = props.step == props.total;
    let details = match (&props.error, &props.operation) {
        (Some(error), _) => html! { <p>{error}</p> },
        (None, Some(operation)) => html! {
            <>
                <p class="sum-active">{format!("Chip {:02} · Node 000", operation.chip + 1)}</p>
                <p>{format!("Local value {}", operation.chip + 1)}</p>
                {for operation.incoming.iter().map(|(source, value)| html! {
                    <p>{format!("+ {value} from Chip {:02}", source + 1)}</p>
                })}
                <p class="sum-result">{format!("{} {} in T", if finished { "Total" } else { "Subtotal" }, operation.sum)}</p>
            </>
        },
        (None, None) => html! {
            <p>{"Each chip starts with its chip number. Step once to add a chip’s value and any subtotals from its neighbors."}</p>
        },
    };
    html! {
        <aside class="sum-callout" aria-label="Distributed sum progress" aria-live="polite">
            <p class="sum-progress">{format!("Step {} of {}", props.step, props.total)}</p>
            {details}
        </aside>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_topologies_reduce_on_f18_nodes() {
        for (topology, count, expected) in [
            (Topology::Single, 1, 1),
            (Topology::Dual, 2, 3),
            (Topology::Ring, 5, 15),
            (Topology::Mesh, 62, 1953),
        ] {
            let edges = topology.layout().links;
            let mut model = Distributed::new(topology);
            let mut completed = vec![false; count];
            while let Some(chip) = model.step() {
                let report = model.last.as_ref().unwrap();
                assert!(!completed[chip]);
                assert!(report.incoming.iter().all(|&(child, _)| {
                    completed[child]
                        && (edges.contains(&(chip, child)) || edges.contains(&(child, chip)))
                }));
                completed[chip] = true;
            }
            assert_eq!(model.steps, count);
            assert!(model.error.is_none());
            assert_eq!(model.last.as_ref().unwrap().chip, 0);
            assert_eq!(model.chip(0).borrow().chip.inspect(sum_node()).t, expected);
        }
    }
}
