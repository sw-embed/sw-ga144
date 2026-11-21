//! Deterministic instruction-level model of one 18-column, 8-row GA144.

mod f18;
mod image;

use std::fmt;

pub use f18::{ENCODING, Fault, Snapshot, WORD_MASK};
pub use image::{Image, ImageError};

use f18::{Node, Request, port_mask};

pub const COLUMNS: usize = 18;
pub const ROWS: usize = 8;
const NODE_COUNT: usize = COLUMNS * ROWS;
const PORT_BITS: [u16; 4] = [8, 4, 2, 1];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeId(usize);

impl NodeId {
    pub fn new(row: usize, column: usize) -> Option<Self> {
        (row < ROWS && column < COLUMNS).then(|| Self(row * COLUMNS + column))
    }

    pub fn from_number(yxx: usize) -> Option<Self> {
        Self::new(yxx / 100, yxx % 100)
    }

    pub fn row(self) -> usize {
        self.0 / COLUMNS
    }

    pub fn column(self) -> usize {
        self.0 % COLUMNS
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}{:02}", self.row(), self.column())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickError {
    pub node: NodeId,
    pub fault: Fault,
}

impl fmt::Display for TickError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "node {}: {:?}", self.node, self.fault)
    }
}

impl std::error::Error for TickError {}

pub struct Ga144 {
    nodes: Vec<Node>,
    links: [[Option<NodeId>; 4]; NODE_COUNT],
}

impl Default for Ga144 {
    fn default() -> Self {
        Self::new()
    }
}

impl Ga144 {
    pub fn new() -> Self {
        Self {
            nodes: (0..NODE_COUNT).map(|_| Node::new()).collect(),
            links: std::array::from_fn(|index| neighbors(NodeId(index))),
        }
    }

    pub fn load_ram(&mut self, node: NodeId, image: &Image) {
        self.nodes[node.0].ram = image.0;
    }

    pub fn load_rom(&mut self, node: NodeId, image: &Image) {
        self.nodes[node.0].rom = Some(image.0);
    }

    /// Starts or redirects the instruction stream, preserving stacks and memory.
    pub fn start(&mut self, node: NodeId, entry: u16) -> Result<(), TickError> {
        if entry > 0x3ff {
            return Err(TickError {
                node,
                fault: Fault::Unmapped(entry),
            });
        }
        self.nodes[node.0].start(entry);
        Ok(())
    }

    pub fn inspect(&self, node: NodeId) -> Snapshot {
        self.nodes[node.0].snapshot()
    }

    pub fn ram(&self, node: NodeId) -> &[u32; 64] {
        &self.nodes[node.0].ram
    }

    /// Advances one fetch or opcode per runnable node and resolves pending I/O.
    /// Returns the number of nodes that advanced. Zero means quiescence.
    pub fn tick(&mut self) -> Result<usize, TickError> {
        let pending = std::array::from_fn(|index| self.nodes[index].request());
        let mut progress = 0;
        for (index, node) in self.nodes.iter_mut().enumerate() {
            let status = port_status(self.links[index], &pending);
            progress += usize::from(node.step(status).map_err(|fault| TickError {
                node: NodeId(index),
                fault,
            })?);
        }

        let pending = std::array::from_fn(|index| self.nodes[index].request());
        let (receives, sends) = self.match_requests(&pending)?;
        for (index, node) in self.nodes.iter_mut().enumerate() {
            if let Some(value) = receives[index] {
                node.complete_port(value);
                progress += 1;
            } else if sends[index] {
                node.complete_port(0);
                progress += 1;
            }
        }
        Ok(progress)
    }

    fn match_requests(
        &self,
        pending: &[Option<Request>; NODE_COUNT],
    ) -> Result<([Option<u32>; NODE_COUNT], [bool; NODE_COUNT]), TickError> {
        let mut receives = [None; NODE_COUNT];
        let mut sends = [false; NODE_COUNT];
        for (writer, request) in pending.iter().enumerate() {
            let Some(Request {
                address,
                write: Some(value),
            }) = *request
            else {
                continue;
            };
            for (port, peer) in self.links[writer].iter().copied().enumerate() {
                let Some(reader) = peer.filter(|_| port_mask(address) & PORT_BITS[port] != 0)
                else {
                    continue;
                };
                let Some(request) = pending[reader.0] else {
                    continue;
                };
                if request.write.is_some() || port_mask(request.address) & PORT_BITS[port] == 0 {
                    continue;
                }
                if receives[reader.0].replace(value).is_some() {
                    return Err(TickError {
                        node: reader,
                        fault: Fault::MultipleWriters(request.address),
                    });
                }
                sends[writer] = true;
            }
        }
        Ok((receives, sends))
    }
}

fn neighbors(node: NodeId) -> [Option<NodeId>; 4] {
    let row = node.row();
    let column = node.column();
    // A physical wire joins two ports of the same name. The outward
    // Left and Up ports on the chip edges have no on-chip peer.
    [
        NodeId::new(row, column ^ 1),
        NodeId::new(row ^ 1, column),
        outer_port(column).and_then(|peer| NodeId::new(row, peer)),
        outer_port(row).and_then(|peer| NodeId::new(peer, column)),
    ]
}

fn outer_port(coordinate: usize) -> Option<usize> {
    let delta = if coordinate.is_multiple_of(2) { -1 } else { 1 };
    coordinate.checked_add_signed(delta)
}

fn port_status(links: [Option<NodeId>; 4], pending: &[Option<Request>; NODE_COUNT]) -> (u32, u32) {
    let mut mask = 0;
    let mut bits = 0;
    for (port, peer) in links.into_iter().enumerate() {
        let Some(peer) = peer else { continue };
        let bit = 15 - 3 * port;
        mask |= 3 << bit;
        let matching =
            pending[peer.0].filter(|request| port_mask(request.address) & PORT_BITS[port] != 0);
        match matching {
            Some(Request { write: None, .. }) => {}
            Some(Request { write: Some(_), .. }) => bits |= 3 << bit,
            None => bits |= 2 << bit,
        }
    }
    (mask, bits)
}
