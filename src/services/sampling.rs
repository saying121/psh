use anyhow::Result;
use perf_event_rs::{
    EventScope, HardwareEvent,
    config::{Cpu, Process},
    sampling::{self, Config, ExtraRecord, OverflowBy, Sampler},
};
use psh_proto::{
    PerfDataProto,
    perf_data_proto::{PerfEvent, PerfFileAttr},
};

#[derive(Default)]
pub struct Profiler {
    samplers: Vec<Sampler>,
}

impl Profiler {
    pub fn new<D: Into<Option<u16>>>(
        process: Process,
        mmap_pages: usize,
        overflow_by: OverflowBy,
        stack_depth: D,
    ) -> Result<Self> {
        let econf = sampling::ExtraConfig {
            comm: true,
            comm_exec: true,
            precise_ip: sampling::SampleIpSkid::Zero,
            inherit: true,
            inherit_stat: true,
            build_id: true,
            // inherit_thread: true,
            extra_record_with_sample_id: true,
            sample_record_fields: sampling::SampleRecordFields {
                id: true,
                sample_id: true,
                time: true,
                ip: true,
                pid_and_tid: true,
                period: true,
                ips: stack_depth.into(),
                ..Default::default()
            },
            extra_record_types: vec![ExtraRecord::Mmap, ExtraRecord::Mmap2],
            ..Default::default()
        };

        let cpu_num = num_cpus::get();
        let mut samplers = Vec::with_capacity(cpu_num);

        for cpu in 0..cpu_num {
            let s = Sampler::new(
                &process,
                &Cpu::Id(cpu as _),
                mmap_pages.next_power_of_two() + 1,
                &Config::extra_new(
                    &HardwareEvent::CpuCycles.into(),
                    &EventScope::all(),
                    &overflow_by,
                    &econf,
                ),
            )?;
            samplers.push(s);
        }

        Ok(Self { samplers })
    }

    /// Get current sampling task data
    pub fn perf_data_proto(&mut self) -> PerfDataProto {
        let file_attrs: Vec<_> = self
            .samplers
            .iter()
            .map(|sampler| {
                let attr = sampler.perf_event_attr();

                let perf_event_attr = attr.into();
                PerfFileAttr {
                    attr: Some(perf_event_attr),
                    ids: vec![],
                }
            })
            .collect();

        let events = self
            .samplers
            .iter_mut()
            .flat_map(|v| v.iter())
            .map(|v| PerfEvent {
                header: None,
                timestamp: None,
                event_type: Some(v.body.into()),
            })
            .collect();

        PerfDataProto {
            file_attrs,
            events,
            event_types: vec![],
            timestamp_sec: None,
            stats: None,
            metadata_mask: vec![],
            tracing_data: None,
            build_ids: vec![],
            uint32_metadata: vec![],
            uint64_metadata: vec![],
            cpu_topology: None,
            numa_topology: vec![],
            pmu_mappings: vec![],
            group_desc: vec![],
            hybrid_topology: vec![],
            string_metadata: None,
        }
    }

    pub fn enable(&self) -> Result<()> {
        for ele in &self.samplers {
            ele.enable()?;
        }
        Ok(())
    }

    pub fn disable(&self) -> Result<()> {
        for ele in &self.samplers {
            ele.disable()?;
        }
        Ok(())
    }
}
