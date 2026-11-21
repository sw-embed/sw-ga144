use std::f64::consts::TAU;

use yew::prelude::*;

const MESH_SIDE: usize = 8;
const RING_CHIPS: usize = 5;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Topology {
    Single,
    Dual,
    Ring,
    Mesh,
}

impl Topology {
    pub(crate) const ALL: [Self; 4] = [Self::Single, Self::Dual, Self::Ring, Self::Mesh];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Single => "Single",
            Self::Dual => "Dual",
            Self::Ring => "Ring",
            Self::Mesh => "Mesh",
        }
    }

    fn caption(self) -> &'static str {
        match self {
            Self::Single => "",
            Self::Dual => "2 chips joined by one point-to-point serial link.",
            Self::Ring => "5 chips in a closed loop, with two serial links per chip.",
            Self::Mesh => "64 chips in an 8×8 grid. Interior chips have four serial links.",
        }
    }

    fn layout(self) -> Layout {
        match self {
            Self::Single => Layout::default(),
            Self::Dual => Layout {
                chips: vec![(25.0, 50.0), (75.0, 50.0)],
                links: vec![(0, 1)],
            },
            Self::Ring => Layout {
                chips: (0..RING_CHIPS)
                    .map(|index| {
                        let angle = TAU * index as f64 / RING_CHIPS as f64;
                        (50.0 + 35.0 * angle.sin(), 50.0 - 35.0 * angle.cos())
                    })
                    .collect(),
                links: (0..RING_CHIPS)
                    .map(|index| (index, (index + 1) % RING_CHIPS))
                    .collect(),
            },
            Self::Mesh => mesh_layout(),
        }
    }
}

#[derive(Default)]
struct Layout {
    chips: Vec<(f64, f64)>,
    links: Vec<(usize, usize)>,
}

fn mesh_layout() -> Layout {
    let mut layout = Layout::default();
    for row in 0..MESH_SIDE {
        for column in 0..MESH_SIDE {
            let index = row * MESH_SIDE + column;
            layout.chips.push((
                12.0 + 76.0 * column as f64 / (MESH_SIDE - 1) as f64,
                12.0 + 76.0 * row as f64 / (MESH_SIDE - 1) as f64,
            ));
            if column + 1 < MESH_SIDE {
                layout.links.push((index, index + 1));
            }
            if row + 1 < MESH_SIDE {
                layout.links.push((index, index + MESH_SIDE));
            }
        }
    }
    layout
}

#[derive(Properties, PartialEq)]
pub(crate) struct NetworkProps {
    pub(crate) topology: Topology,
    pub(crate) selected: usize,
    pub(crate) on_select: Callback<usize>,
}

#[function_component(Network)]
pub(crate) fn network(props: &NetworkProps) -> Html {
    let top_view = use_state(|| false);
    let toggle_view = {
        let top_view = top_view.clone();
        Callback::from(move |_| top_view.set(!*top_view))
    };
    let layout = props.topology.layout();
    let mesh = props.topology == Topology::Mesh;
    html! {
        <section class="panel network-panel" aria-labelledby="network-title">
            <div class="panel-title">
                <h2 id="network-title">{format!("{} topology", props.topology.title())}</h2>
                <button type="button" aria-pressed={top_view.to_string()} onclick={toggle_view}>{"Top view"}</button>
            </div>
            <div class="network-content">
                <div class="network-copy">
                    <p>{props.topology.caption()}</p>
                    <p>{"The high-speed serial links are placeholders. Choose a chip to inspect its 144 nodes."}</p>
                    <output aria-live="polite">{format!("Chip {:02} selected", props.selected + 1)}</output>
                </div>
                <div class={classes!("network-viewport", mesh.then_some("mesh-view"))}>
                    <div class={classes!("network-board", (*top_view).then_some("top-view"))}>
                        {for layout.links.iter().map(|&(from, to)| {
                            let (x, y) = layout.chips[from];
                            let (end_x, end_y) = layout.chips[to];
                            let dx = end_x - x;
                            let dy = end_y - y;
                            let style = format!(
                                "left:{x}%;top:{y}%;width:{}%;transform:rotate({}rad)",
                                dx.hypot(dy),
                                dy.atan2(dx)
                            );
                            html! { <span class="network-link" style={style} aria-hidden="true"/> }
                        })}
                        {for layout.chips.iter().enumerate().map(|(index, &(x, y))| {
                            let select = props.on_select.reform(move |_| index);
                            let label = format!("Chip {:02}", index + 1);
                            html! {
                                <button key={index} type="button" class="package" style={format!("left:{x}%;top:{y}%")}
                                    aria-label={label.clone()} title={label} aria-pressed={(index == props.selected).to_string()}
                                    onclick={select}/>
                            }
                        })}
                    </div>
                </div>
            </div>
        </section>
    }
}
