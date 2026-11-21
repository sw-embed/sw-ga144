use std::f64::consts::TAU;

use yew::prelude::*;

use crate::distributed::Operation;

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

    pub(crate) fn participants(self) -> usize {
        match self {
            Self::Single => 1,
            Self::Dual => 2,
            Self::Ring => RING_CHIPS,
            Self::Mesh => MESH_SIDE * MESH_SIDE - 2,
        }
    }

    pub(crate) fn layout(self) -> Layout {
        match self {
            Self::Single => Layout {
                chips: vec![(50.0, 50.0)],
                links: Vec::new(),
            },
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
pub(crate) struct Layout {
    pub(crate) chips: Vec<(f64, f64)>,
    pub(crate) links: Vec<(usize, usize)>,
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
    pub(crate) operation: Option<Operation>,
    pub(crate) controls: Html,
    #[prop_or_default]
    pub(crate) children: Children,
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
    let participants = props.topology.participants();
    html! {
        <section class="panel network-panel" aria-labelledby="network-title">
            <div class="panel-title">
                <h2 id="network-title">{format!("{} · Distributed sum", props.topology.title())}</h2>
                <div class="controls">
                    <button type="button" aria-pressed={top_view.to_string()} onclick={toggle_view}>{"Top view"}</button>
                    {props.controls.clone()}
                </div>
            </div>
            <div class="network-content">
                <div class="network-copy">
                    <p>{props.topology.caption()}</p>
                    <p>{"Transfers use a conceptual transport. Choose a chip to inspect its 144 nodes."}</p>
                    {if mesh { html! { <p>{"Chip 63 and Chip 64 are idle in this demo."}</p> } } else { Html::default() }}
                    <output aria-live="polite">{format!("Chip {:02} selected", props.selected + 1)}</output>
                    {for props.children.iter()}
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
                            let carrying = props.operation.as_ref().is_some_and(|operation| {
                                operation.incoming.iter().any(|&(child, _)| {
                                    (from, to) == (operation.chip, child) || (to, from) == (operation.chip, child)
                                })
                            });
                            let idle = from >= participants || to >= participants;
                            html! { <span class={classes!("network-link", carrying.then_some("carrying"), idle.then_some("idle-link"))}
                                style={style} aria-hidden="true"/> }
                        })}
                        {for layout.chips.iter().enumerate().map(|(index, &(x, y))| {
                            let select = props.on_select.reform(move |_| index);
                            let active = props.operation.as_ref().is_some_and(|operation| operation.chip == index);
                            let idle = index >= participants;
                            let label = format!("Chip {:02}", index + 1);
                            let status = if active { Some("active chip") } else if idle { Some("idle chip") } else { None };
                            html! {
                                <button key={index} type="button" class={classes!("package", active.then_some("processing"), idle.then_some("idle-package"))} style={format!("left:{x}%;top:{y}%")}
                                    aria-label={label.clone()} aria-description={status} title={label} aria-pressed={(index == props.selected).to_string()}
                                    onclick={select}/>
                            }
                        })}
                    </div>
                </div>
            </div>
        </section>
    }
}
