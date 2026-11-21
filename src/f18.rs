pub const WORD_MASK: u32 = 0x3ffff;
pub const ENCODING: u32 = 0x15555;
pub const IO: u16 = 0x15d;
const SIGN: u32 = 0x20000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Request {
    pub address: u16,
    pub write: Option<u32>,
}

#[derive(Clone, Copy)]
enum BusAction {
    FetchInstruction,
    Memory(u8),
}

#[derive(Clone, Copy)]
struct BusCycle {
    action: BusAction,
    request: Request,
}

#[derive(Clone, Copy)]
struct Stack<const HEAD: usize> {
    registers: [u32; HEAD],
    ring: [u32; 8],
    top: usize,
}

impl<const HEAD: usize> Stack<HEAD> {
    fn new() -> Self {
        Self {
            registers: [0; HEAD],
            ring: [0; 8],
            top: 0,
        }
    }

    fn peek(&self, depth: usize) -> u32 {
        self.registers[depth]
    }

    fn replace_top(&mut self, value: u32) {
        self.registers[0] = value & WORD_MASK;
    }

    fn push(&mut self, value: u32) {
        self.top = (self.top + 1) & 7;
        self.ring[self.top] = self.registers[HEAD - 1];
        self.registers.rotate_right(1);
        self.replace_top(value);
    }

    fn pop(&mut self) -> u32 {
        let value = self.peek(0);
        self.registers.rotate_left(1);
        self.registers[HEAD - 1] = self.ring[self.top];
        self.top = (self.top + 7) & 7;
        value
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub p: u16,
    pub a: u32,
    pub b: u16,
    pub t: u32,
    pub s: u32,
    pub r: u32,
    pub instruction: u32,
    /// The next slot, or 4 when an instruction word must be fetched.
    pub slot: u8,
    pub active: bool,
    pub waiting_address: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    Unmapped(u16),
    ReadOnly(u16),
    MultipleWriters(u16),
}

pub(crate) struct Node {
    pub ram: [u32; 64],
    pub rom: Option<[u32; 64]>,
    pub active: bool,
    p: u16,
    a: u32,
    b: u16,
    io: u32,
    carry: u32,
    i: u32,
    slot: u8,
    data: Stack<2>,
    returns: Stack<1>,
    cycle: Option<BusCycle>,
}

impl Node {
    pub fn new() -> Self {
        Self {
            ram: [ENCODING; 64],
            rom: None,
            active: false,
            p: 0,
            a: 0,
            b: IO,
            io: ENCODING,
            carry: 0,
            i: 0,
            slot: 4,
            data: Stack::new(),
            returns: Stack::new(),
            cycle: None,
        }
    }

    pub fn start(&mut self, p: u16) {
        self.p = p;
        self.slot = 4;
        self.cycle = None;
        self.active = true;
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            p: self.p,
            a: self.a,
            b: self.b,
            t: self.data.peek(0),
            s: self.data.peek(1),
            r: self.returns.peek(0),
            instruction: self.i,
            slot: self.slot,
            active: self.active,
            waiting_address: self.request().map(|request| request.address),
        }
    }

    pub fn request(&self) -> Option<Request> {
        self.cycle.map(|cycle| cycle.request)
    }

    /// Completes one fetch or slot. Port operations suspend until the
    /// chip scheduler completes the pending cycle.
    pub fn step(&mut self, port_status: (u32, u32)) -> Result<bool, Fault> {
        if !self.active || self.cycle.is_some() {
            return Ok(false);
        }

        if self.slot == 4 {
            return self.access(BusAction::FetchInstruction, self.p, None, port_status);
        }

        let raw = self.i ^ ENCODING;
        let shift = [13, 8, 3, 0][usize::from(self.slot)];
        let op = if self.slot == 3 {
            ((raw & 7) << 2) as u8
        } else {
            ((raw >> shift) & 31) as u8
        };
        match op {
            0..=7 => self.control(op),
            8..=15 => {
                let address = match op & 3 {
                    0 => self.p,
                    2 => self.b,
                    _ => (self.a & 0x3ff) as u16,
                };
                let write = (op >= 12).then(|| self.data.peek(0));
                return self.access(BusAction::Memory(op), address, write, port_status);
            }
            _ => {
                self.alu(op);
                self.slot += 1;
            }
        }
        Ok(true)
    }

    fn access(
        &mut self,
        action: BusAction,
        address: u16,
        write: Option<u32>,
        (status_mask, status_bits): (u32, u32),
    ) -> Result<bool, Fault> {
        let physical = address & 0x1ff;
        let value = match (physical, write) {
            (0..=0x7f, Some(value)) => {
                self.ram[usize::from(physical & 63)] = value;
                0
            }
            (0..=0x7f, None) => self.ram[usize::from(physical & 63)],
            (0x80..=0xff, Some(_)) => return Err(Fault::ReadOnly(address)),
            (0x80..=0xff, None) => {
                let rom = self.rom.as_ref().ok_or(Fault::Unmapped(address))?;
                rom[usize::from(physical & 63)]
            }
            (IO, Some(value)) => {
                self.io = value;
                0
            }
            (IO, None) => ((!self.io & WORD_MASK) & !status_mask) | status_bits,
            _ if port_mask(physical) != 0 => {
                self.cycle = Some(BusCycle {
                    action,
                    request: Request {
                        address: physical,
                        write,
                    },
                });
                return Ok(false);
            }
            _ => return Err(Fault::Unmapped(address)),
        };
        self.finish(action, value);
        Ok(true)
    }

    pub fn complete_port(&mut self, value: u32) {
        let cycle = self
            .cycle
            .take()
            .expect("scheduler matched a pending cycle");
        self.finish(cycle.action, value);
    }

    fn finish(&mut self, action: BusAction, value: u32) {
        match action {
            BusAction::FetchInstruction => {
                self.i = value;
                self.p = increment(u32::from(self.p)) as u16;
                self.slot = 0;
            }
            BusAction::Memory(op) => {
                if op < 12 {
                    self.data.push(value);
                } else {
                    self.data.pop();
                }
                match op & 3 {
                    0 => self.p = increment(u32::from(self.p)) as u16,
                    1 => self.a = increment(self.a),
                    _ => {}
                }
                self.slot += 1;
            }
        }
    }

    fn control(&mut self, op: u8) {
        let mask = [0x3ff, 0xff, 7, 0][usize::from(self.slot)];
        let dest = (self.p & !mask & !0x100) | (self.i as u16 & mask);
        let r = self.returns.peek(0);
        let t = self.data.peek(0);

        if op == 4 {
            if r == 0 {
                self.returns.pop();
                self.slot += 1;
            } else {
                self.returns.replace_top(r - 1);
                self.slot = 0;
            }
            return;
        }

        match op {
            0 => self.p = (self.returns.pop() & 0x3ff) as u16,
            1 => {
                self.returns.replace_top(u32::from(self.p));
                self.p = (r & 0x3ff) as u16;
            }
            2 => self.p = dest,
            3 => {
                self.returns.push(u32::from(self.p));
                self.p = dest;
            }
            5 if r == 0 => {
                self.returns.pop();
            }
            5 => {
                self.returns.replace_top(r - 1);
                self.p = dest;
            }
            6 if t == 0 => self.p = dest,
            7 if t & SIGN == 0 => self.p = dest,
            _ => {}
        }
        self.slot = 4;
    }

    fn alu(&mut self, op: u8) {
        let t = self.data.peek(0);
        let s = self.data.peek(1);
        match op {
            16 => self.multiply_step(s, t),
            17 => self.data.replace_top(t << 1),
            18 => self.data.replace_top((signed(t) >> 1) as u32),
            19 => self.data.replace_top(!t),
            20..=22 => {
                let result = match op {
                    20 => s + t + self.carry_in(),
                    21 => s & t,
                    _ => s ^ t,
                };
                if op == 20 && self.extended() {
                    self.carry = result >> 18;
                }
                self.data.pop();
                self.data.replace_top(result);
            }
            23 => {
                self.data.pop();
            }
            24 => self.data.push(t),
            25 => self.data.push(self.returns.pop()),
            26 => self.data.push(s),
            27 => self.data.push(self.a),
            28 => {}
            29 => self.returns.push(self.data.pop()),
            30 => self.b = (self.data.pop() & 0x1ff) as u16,
            31 => self.a = self.data.pop(),
            _ => unreachable!("ALU opcode is 16..31"),
        }
    }

    fn multiply_step(&mut self, s: u32, t: u32) {
        let mut high = signed(t);
        if self.a & 1 != 0 {
            let carry = self.carry_in();
            high += signed(s) + carry as i32;
            if self.extended() {
                self.carry = (s + t + carry) >> 18;
            }
        }
        self.a = (self.a >> 1) | ((high as u32 & 1) << 17);
        self.data.replace_top((high >> 1) as u32);
    }

    fn extended(&self) -> bool {
        self.p & 0x200 != 0
    }

    fn carry_in(&self) -> u32 {
        if self.extended() { self.carry } else { 0 }
    }
}

fn signed(value: u32) -> i32 {
    ((value << 14) as i32) >> 14
}

fn increment(value: u32) -> u32 {
    if value & 0x100 != 0 {
        value
    } else {
        (value & !0x7f) | ((value + 1) & 0x7f)
    }
}

pub(crate) fn port_mask(address: u16) -> u16 {
    if address & 0x10f != 0x105 {
        return 0;
    }
    ((address >> 4) & 15) ^ 5
}
