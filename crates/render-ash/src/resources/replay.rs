use std::borrow::Cow;

use pulz_render::{
    backend::GpuResource,
    pipeline::{
        BindGroupLayout, BindGroupLayoutDescriptor, ComputePipeline, ComputePipelineDescriptor,
        GraphicsPass, GraphicsPassDescriptor, GraphicsPipeline, GraphicsPipelineDescriptor,
        PipelineLayout, PipelineLayoutDescriptor, RayTracingPipeline, RayTracingPipelineDescriptor,
    },
    shader::{ShaderModule, ShaderModuleDescriptor},
    utils::serde_slots::SlotDeserializationMapper,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use slotmap::Key;

use super::AshResources;
use crate::{Error, Result};

#[derive(Clone, Debug, Serialize, Deserialize)]
enum ReplayResourceDescr<'a, 'b> {
    GraphicsPass(Cow<'a, GraphicsPassDescriptor>),
    #[serde(borrow)]
    ShaderModule(Cow<'a, ShaderModuleDescriptor<'b>>),
    BindGroupLayout(Cow<'a, BindGroupLayoutDescriptor<'b>>),
    PipelineLayout(Cow<'a, PipelineLayoutDescriptor<'b>>),
    GraphicsPipeline(Cow<'a, GraphicsPipelineDescriptor<'b>>),
    ComputePipeline(Cow<'a, ComputePipelineDescriptor<'b>>),
    RayTracingPipeline(Cow<'a, RayTracingPipelineDescriptor<'b>>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ReplayResourceDescrEntry<'a, 'b>(
    u64,
    #[serde(borrow)] ReplayResourceDescr<'a, 'b>,
);

pub(super) trait AsResourceRecord: GpuResource {
    fn as_record<'a, 'b: 'a>(
        &self,
        descr: &'a Self::Descriptor<'b>,
    ) -> ReplayResourceDescrEntry<'a, 'b>;
}

macro_rules! define_replay_resources {
    ($($r:ident),*) => {

        impl ReplayResourceDescr<'_, '_> {
            fn create(
                &self,
                res: &mut AshResources,
                id: u64,
                mapper: &mut SlotDeserializationMapper,
            ) -> Result<()> {
                match self {
                    $(
                        Self::$r(d) => {
                            let key = res.create::<$r>(d)?;
                            mapper.define(id, key);
                        },
                    )*
                }
                Ok(())
            }
        }

        $(
            impl AsResourceRecord for $r {
                fn as_record<'a, 'b: 'a>(&self, descr: &'a Self::Descriptor<'b>) -> ReplayResourceDescrEntry<'a, 'b> {
                    ReplayResourceDescrEntry(self.data().as_ffi(), ReplayResourceDescr::$r(Cow::Borrowed(descr)))
                }
            }
        )*
    };
}

define_replay_resources!(
    GraphicsPass,
    ShaderModule,
    BindGroupLayout,
    PipelineLayout,
    GraphicsPipeline,
    ComputePipeline,
    RayTracingPipeline
);

pub(super) trait RecordResource: 'static {
    fn record(&mut self, record: ReplayResourceDescrEntry<'_, '_>) -> Result<()>;

    fn end(&mut self) -> Result<()>;
}

pub struct NoopRecorder;

impl RecordResource for NoopRecorder {
    fn record(&mut self, _record: ReplayResourceDescrEntry<'_, '_>) -> Result<()> {
        Ok(())
    }
    fn end(&mut self) -> Result<()> {
        Ok(())
    }
}

struct Recorder<S: Serializer>(Option<S::SerializeSeq>);

impl<S: Serializer> RecordResource for Recorder<S>
where
    S: 'static,
{
    fn record(&mut self, record: ReplayResourceDescrEntry<'_, '_>) -> Result<()> {
        use serde::ser::SerializeSeq;
        let Some(seq) = &mut self.0 else {
            return Ok(());
        };
        if let Err(e) = seq.serialize_element(&record) {
            Err(Error::SerializationError(Box::new(e)))
        } else {
            Ok(())
        }
    }
    fn end(&mut self) -> Result<()> {
        use serde::ser::SerializeSeq;
        let Some(seq) = self.0.take() else {
            return Ok(());
        };

        if let Err(e) = seq.end() {
            Err(Error::SerializationError(Box::new(e)))
        } else {
            Ok(())
        }
    }
}

impl<S: Serializer> Drop for Recorder<S> {
    fn drop(&mut self) {
        use serde::ser::SerializeSeq;
        let Some(seq) = self.0.take() else {
            return;
        };
        seq.end()
            .expect("recording not ended, and produced an error");
    }
}

impl AshResources {
    #[inline]
    pub fn replay<'de, D>(&mut self, deserializer: D) -> Result<()>
    where
        D: Deserializer<'de>,
        D::Error: 'static,
    {
        struct VisitResources<'l>(&'l mut AshResources, &'l mut Option<Error>);
        impl<'de> serde::de::Visitor<'de> for VisitResources<'_> {
            type Value = ();
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a sequence")
            }

            fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                SlotDeserializationMapper::with(|mapper| {
                    while let Some(ReplayResourceDescrEntry(id, elem)) = seq.next_element()? {
                        if let Err(error) = elem.create(self.0, id, mapper) {
                            *self.1 = Some(error);
                            return Err(serde::de::Error::custom("Unable to create resource"));
                        }
                    }
                    Ok(())
                })
            }
        }
        let mut error = None;
        let result = deserializer.deserialize_seq(VisitResources(self, &mut error));
        if let Some(error) = error {
            return Err(error);
        }
        if let Err(error) = result {
            return Err(Error::DeserializationError(Box::new(error)));
        }
        Ok(())
    }

    pub fn start_recording<S: Serializer + 'static>(
        &mut self,
        serializer: S,
    ) -> Result<(), S::Error> {
        assert!(
            self.record.is_none(),
            "there is already an active recording session"
        );
        let seq = serializer.serialize_seq(None)?;
        self.record.replace(Box::new(Recorder::<S>(Some(seq))));
        Ok(())
    }

    pub fn end_recording(&mut self) -> Result<bool> {
        if let Some(mut record) = self.record.take() {
            record.end()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
