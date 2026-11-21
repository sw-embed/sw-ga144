use sw_ga144::{COLUMNS, Ga144, Image, NodeId, ROWS};
use web_sys::{HtmlSelectElement, HtmlTextAreaElement};
use yew::prelude::*;

mod demos;
use demos::{Demo, MULTIPLE_NODES, SINGLE_NODE};

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
    fn new(demo: &Demo) -> Self {
        let mut machine = Self {
            chip: Ga144::new(),
            ticks: 0,
            status: "Ready".into(),
            selected: NodeId::from_number(demo.result_node).expect("valid demo result node"),
            programs: Vec::new(),
        };
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
    let help_open = use_state(|| false);
    let toggle_help = {
        let help_open = help_open.clone();
        Callback::from(move |_| help_open.set(!*help_open))
    };
    let demo = demos::all().nth(*selection).expect("valid demo index");
    let machine = use_mut_ref(|| Machine::new(&SINGLE_NODE[0]));
    let redraw = use_state(|| 0);
    let editor = use_node_ref();
    let entry_field = use_node_ref();

    let change_demo = {
        let machine = machine.clone();
        let redraw = redraw.clone();
        let selection = selection.clone();
        Callback::from(move |event: Event| {
            let select = event.target_unchecked_into::<HtmlSelectElement>();
            if let Ok(index) = select.value().parse::<usize>()
                && let Some(demo) = demos::all().nth(index)
            {
                *machine.borrow_mut() = Machine::new(demo);
                selection.set(index);
                redraw.set(*redraw + 1);
            }
        })
    };
    let reset = {
        let machine = machine.clone();
        let redraw = redraw.clone();
        Callback::from(move |_| {
            *machine.borrow_mut() = Machine::new(demo);
            redraw.set(*redraw + 1);
        })
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

    let state = machine.borrow();
    let selected = state.selected;
    let registers = state.chip.inspect(selected);
    let node_grid = (0..ROWS).rev().flat_map(|row| (0..COLUMNS).map(move |col| (row, col))).map(|(row, col)| {
        let id = NodeId::new(row, col).unwrap();
        let machine = machine.clone();
        let redraw = redraw.clone();
        let status = state.chip.inspect(id);
        let mode = if status.waiting_address.is_some() { "waiting" } else if status.active { "active" } else { "idle" };
        html! {
            <button type="button" class={classes!("node", mode)} aria-pressed={(id == selected).to_string()}
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
                <div class="demo-picker">
                    <label for="demo">{"Demo"}</label>
                    <select id="demo" onchange={change_demo} aria-describedby="demo-description">
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
                    </select>
                </div>
                <button type="button" aria-expanded={help_open.to_string()} aria-controls="demo-help" onclick={toggle_help}>{"Help"}</button>
                </div>
            </header>
            <section id="demo-help" class="panel help" hidden={!*help_open} aria-labelledby="help-heading">
                <h2 id="help-heading">{"How to use GA144 Studio"}</h2>
                <ol>
                    <li>{"Pick a demo. Studio loads its programs and selects the node where the answer will appear."}</li>
                    <li>{"Click Run demo to advance the chip, or Step to advance one tick. Reset reloads the selected demo. Click any square to inspect that node."}</li>
                    <li>{"Look at T for the demo result. T is the top of the node’s data stack. The inspector also shows its source, RAM, and other registers."}</li>
                    <li>{"For your own program, run "}<code>{"./assemble program.f18"}</code>{", choose a node, and paste the listing under Load an assembler listing. Use entry address 0 for a program that starts at RAM address 0."}</li>
                </ol>
                <p>{"The inspector displays results directly. You can also send values between simulated nodes."}</p>
                <p>{"This demo is conceptual and based on public documentation. It has not been validated against actual GreenArrays hardware or software."}</p>
            </section>
            <section class="panel">
                <div class="panel-title">
                    <h2>{"Chip map"}</h2>
                    <div class="controls">
                        <button onclick={reset}>{"Reset"}</button>
                        <button onclick={advance(1)}>{"Step"}</button>
                        <button class="primary" onclick={advance(demo.ticks)}>{"Run demo"}</button>
                    </div>
                </div>
                <p id="demo-description">{demo.description}</p>
                <div class="scroll"><div class="chip">{for node_grid}</div></div>
                <div class="legend">
                    <span><i class="active"></i>{"Active"}</span><span><i class="waiting"></i>{"Waiting"}</span><span><i class="idle"></i>{"Idle"}</span>
                    <output aria-live="polite">{format!("Tick {} · {}", state.ticks, state.status)}</output>
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
                        <span class="detail">{if instructions.is_some() { "F18 source" } else { "Assembler listing" }}</span>
                    </div>
                    {if let Some(source) = instructions { html! {<pre>{source}</pre>} }
                      else if program.is_some() { html! {<pre>{listing}</pre>} }
                      else { html! {<p>{"This node has no loaded program."}</p>} }}
                    <details key={format!("listing-{selected}")}><summary>{"Load an assembler listing"}</summary>
                        <label for="listing">{"Decimal address and word pairs"}</label>
                        <textarea id="listing" ref={editor} value={listing.to_owned()} rows="5" spellcheck="false"/>
                        <div class="controls loader">
                            <label for="entry">{"Entry address"}</label>
                            <input id="entry" type="number" min="0" max="1023" ref={entry_field} value={entry.to_string()}/>
                            <button onclick={load}>{format!("Load node {selected}")}</button>
                        </div>
                    </details>
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
