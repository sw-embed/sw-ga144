pub(crate) struct Demo {
    pub title: &'static str,
    pub description: &'static str,
    pub result_node: usize,
    pub ticks: usize,
    pub programs: &'static [Example],
}

pub(crate) struct Example {
    pub node: usize,
    pub entry: u16,
    pub source: &'static str,
    pub listing: &'static str,
}

macro_rules! example {
    ($node:literal, $entry:literal, $name:literal) => {
        Example {
            node: $node,
            entry: $entry,
            source: include_str!(concat!("../../examples/", $name, ".f18")),
            listing: include_str!(concat!(env!("OUT_DIR"), "/", $name, ".img")),
        }
    };
}

pub(crate) const SINGLE_NODE: &[Demo] = &[
    Demo {
        title: "Double 21",
        description: "Node 000 doubles 21, then loops in place with 42 in T. Reset to run it from the start.",
        result_node: 0,
        ticks: 8,
        programs: &[example!(0, 1, "double")],
    },
    Demo {
        title: "Sum 1 through 5",
        description: "Node 000 loops five times, adding the next number to its running total. Look for 15 in T.",
        result_node: 0,
        ticks: 80,
        programs: &[example!(0, 0, "sum")],
    },
];

pub(crate) const MULTIPLE_NODES: &[Demo] = &[
    Demo {
        title: "Send 42",
        description: "Node 000 sends 42 over a shared port. Node 001 receives it in T.",
        result_node: 1,
        ticks: 20,
        programs: &[example!(0, 0, "send"), example!(1, 0, "receive")],
    },
    Demo {
        title: "Double and forward",
        description: "Node 000 sends 42 to node 001, which doubles it and sends 84 to node 002. Look for 84 in node 002’s T.",
        result_node: 2,
        ticks: 32,
        programs: &[
            example!(0, 0, "send"),
            example!(1, 0, "relay-double"),
            example!(2, 0, "receive-relay"),
        ],
    },
];

pub(crate) fn all() -> impl Iterator<Item = &'static Demo> {
    SINGLE_NODE.iter().chain(MULTIPLE_NODES)
}
