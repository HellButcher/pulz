use std::fmt::Debug;

use crate::{
    prelude::Schedule,
    schedule::{Layer, SystemId},
    system::BoxedSystem,
};

impl Debug for Layer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == !0 {
            write!(f, "Layer(-)")
        } else {
            write!(f, "Layer({})", self.0)
        }
    }
}

impl Debug for SystemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_undefined() {
            write!(f, "NodeId(-)")
        } else {
            write!(f, "NodeId({})", self.0)
        }
    }
}

#[derive(Debug)]
#[allow(unused)] // false positive: used for Debug
struct System<'s> {
    id: SystemId,
    system: &'s BoxedSystem,
    layer: Layer,
    dependent: Layer,
}
pub struct SystemsDebug<'a> {
    systems: &'a [BoxedSystem],
    topo_layers: &'a [Vec<SystemId>],
    dependent_layers: &'a [Layer],
}

struct LayerSystemsDebug<'a> {
    data: &'a SystemsDebug<'a>,
    layer: Layer,
}

struct LayerDebug<'a> {
    data: &'a SystemsDebug<'a>,
    layer: Layer,
}

impl Debug for LayerSystemsDebug<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let layer = self
            .data
            .topo_layers
            .get(self.layer.0)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut l = f.debug_list();
        for &id in layer.iter() {
            l.entry(&System {
                id,
                system: &self.data.systems[id.0],
                layer: self.layer,
                dependent: self
                    .data
                    .dependent_layers
                    .get(id.0)
                    .copied()
                    .unwrap_or(Layer::UNDEFINED),
            });
        }
        l.finish()
    }
}

impl Debug for LayerDebug<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("Layer");
        s.field("id", &self.layer);
        s.field(
            "systems",
            &LayerSystemsDebug {
                data: self.data,
                layer: self.layer,
            },
        );
        s.finish()
    }
}

impl<'a> SystemsDebug<'a> {
    pub fn from_schedule(schedule: &'a Schedule) -> Self {
        Self {
            systems: &schedule.systems,
            topo_layers: &schedule.ordered_layers,
            dependent_layers: &schedule.system_dependent_layers,
        }
    }

    pub fn system(&self, id: SystemId) -> Option<impl Debug + '_> {
        if id.0 >= self.systems.len() {
            return None;
        }
        let system = &self.systems[id.0];
        Some(System {
            id,
            system,
            layer: Layer(
                self.topo_layers
                    .iter()
                    .position(|l| l.contains(&id))
                    .unwrap_or(!0),
            ),
            dependent: self
                .dependent_layers
                .get(id.0)
                .copied()
                .unwrap_or(Layer::UNDEFINED),
        })
    }

    pub fn layer(&self, layer: Layer) -> Option<impl Debug + '_> {
        if layer.0 >= self.topo_layers.len() {
            return None;
        }
        Some(LayerDebug { data: self, layer })
    }
}

impl Debug for SystemsDebug<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.topo_layers.is_empty() {
            let mut s = f.debug_list();
            for i in 0..self.systems.len() {
                let id = SystemId(i);
                if let Some(system_debug) = self.system(id) {
                    s.entry(&system_debug);
                }
            }
            s.finish()
        } else {
            let mut s = f.debug_list();
            for i in 0..self.topo_layers.len() {
                let layer = Layer(i);
                if let Some(layer_debug) = self.layer(layer) {
                    s.entry(&layer_debug);
                }
            }
            s.finish()
        }
    }
}

impl Debug for Schedule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_tuple("Schedule");
        s.field(&SystemsDebug::from_schedule(self));
        s.finish()
    }
}
