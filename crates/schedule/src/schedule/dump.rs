use std::{backtrace::Backtrace, path::Path, str::FromStr};

use crate::prelude::Schedule;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum ScheduleDumpFormat {
    #[default]
    Debug,
    Dot,
}

impl FromStr for ScheduleDumpFormat {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if "debug".eq_ignore_ascii_case(s) {
            Ok(Self::Debug)
        } else if "dot".eq_ignore_ascii_case(s) {
            Ok(Self::Dot)
        } else {
            Err("Unknown schedule dump format")
        }
    }
}

impl Schedule {
    /// Dumps the schedule to a file specified by the `PULZ_DUMP_SCHEDULE` environment variable, if it is set.
    /// The dump-format can be specified by the `PULZ_DUMP_SCHEDULE_FORMAT` environment variable and defaults to `debug`.
    pub fn dump_if_env(&self) -> std::io::Result<()> {
        let Some(path) = std::env::var_os("PULZ_DUMP_SCHEDULE") else {
            return Ok(());
        };
        let format: ScheduleDumpFormat = std::env::var("PULZ_DUMP_SCHEDULE_FORMAT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_default();
        let backtrace = Backtrace::force_capture();
        self.dump_file(format, Path::new(&path), Some(&backtrace))
    }

    pub fn dump_file(
        &self,
        format: ScheduleDumpFormat,
        path: &Path,
        backtrace: Option<&Backtrace>,
    ) -> std::io::Result<()> {
        let mut f = std::fs::File::create(path)?;
        self.dump(format, &mut f, backtrace)
    }

    pub fn dump(
        &self,
        format: ScheduleDumpFormat,
        out: &mut dyn std::io::Write,
        backtrace: Option<&Backtrace>,
    ) -> std::io::Result<()> {
        match format {
            ScheduleDumpFormat::Debug => {
                self.dump_debug(out, backtrace)?;
            }
            ScheduleDumpFormat::Dot => {
                self.dump_dot(out, backtrace)?;
            }
        }
        Ok(())
    }

    fn write_info(
        &self,
        out: &mut dyn std::io::Write,
        backtrace: Option<&Backtrace>,
    ) -> std::io::Result<()> {
        writeln!(
            out,
            "// Debug Dump for schedule created on {:?}",
            std::time::Instant::now()
        )?;
        if let Some(backtrace) = backtrace {
            self.write_backtrace(out, backtrace)?;
        }
        Ok(())
    }

    fn write_backtrace(
        &self,
        out: &mut dyn std::io::Write,
        backtrace: &Backtrace,
    ) -> std::io::Result<()> {
        writeln!(out, "/*\n  Backtrace\n  =========\n{backtrace}\n*/")?;
        Ok(())
    }

    fn dump_debug(
        &self,
        out: &mut dyn std::io::Write,
        backtrace: Option<&Backtrace>,
    ) -> std::io::Result<()> {
        self.write_info(out, backtrace)?;
        writeln!(out)?;
        writeln!(out, "{self:#?}")?;
        Ok(())
    }

    fn dump_dot(
        &self,
        out: &mut dyn std::io::Write,
        backtrace: Option<&Backtrace>,
    ) -> std::io::Result<()> {
        self.write_info(out, backtrace)?;
        writeln!(out)?;
        writeln!(out, " /*\n  Schedule\n  =========\n{self:#?}")?;
        writeln!(out)?;
        writeln!(out, "TODO!")?;
        //TODO: self.write_dot(out, None)?;
        Ok(())
    }

    /*
    pub fn write_dot(
        &self,
        w: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        writeln!(w, "digraph system {{")?;
        writeln!(
            w,
            "  graph [ranksep=0.5,overlap=scale,splines=true,compound=true];"
        )?;
        writeln!(w, "  start [shape=point];\n")?;

        if self.atom.is_dirty() {
            for (s, system) in self.systems.iter().enumerate() {
                writeln!(w, "  s{s} [shape=box, label=\"{}\"];", system.type_name())?;
            }
        } else {
            for (i, group) in self.ordered_task_groups.iter().enumerate() {
                match group {
                    &TaskGroup::Exclusive(s) => {
                        writeln!(
                            w,
                            "  s{s} [shape=box, label=\"{}\"];",
                            self.systems[s].type_name()
                        )?;
                        if i == 0 {
                            writeln!(w, "  start -> s{s} [style=dashed];")?;
                        }
                    }
                    TaskGroup::Concurrent(entries) => {
                        writeln!(w, "  subgraph cluster_{i} {{")?;
                        for &(s, _) in entries {
                            writeln!(w, "    s{s} [label=\"{}\"];", self.systems[s].type_name())?;
                        }
                        writeln!(w, "    style=dashed;")?;
                        writeln!(w, "  }}")?;

                        let first_in_group = entries.first().unwrap().0;
                        if i == 0 {
                            writeln!(
                                w,
                                "  start -> s{first_in_group} [style=dashed, lhead=cluster_{i}];"
                            )?;
                        } else if let TaskGroup::Exclusive(prev) = self.ordered_task_groups[i - 1] {
                            writeln!(
                                w,
                                "  s{prev} -> s{first_in_group} [style=dashed, lhead=cluster_{i}];"
                            )?;
                        }
                        let next = match self.ordered_task_groups.get(i + 1) {
                            Some(TaskGroup::Exclusive(next)) => *next,
                            Some(TaskGroup::Concurrent(entries)) => entries.first().unwrap().0,
                            None => self.systems.len(),
                        };
                        for &(s, e) in entries {
                            if e >= entries.len() {
                                writeln!(w, "  s{s} -> s{next} [style=dashed];")?;
                            } else {
                                let next = entries[e].0;
                                writeln!(w, "  s{s} -> s{next};")?;
                            }
                        }
                    }
                }
            }
        }

        let end = self.systems.len();
        writeln!(w, "  s{end} [shape=point];")?;
        if self.atom.is_dirty() {
            writeln!(w, "  start -> s{end} [style=dashed];")?;
        } else if let Some(&TaskGroup::Exclusive(prev)) = self.ordered_task_groups.last() {
            writeln!(w, "  s{prev} -> s{end} [style=dashed];")?;
        }

        // legend
        writeln!(w, "  subgraph cluster_legend {{")?;
        writeln!(w, "    x0 [shape=point,xlabel=\"Start\"];")?;
        writeln!(w, "    x1 [shape=box, label=\"Exclusive\"];")?;
        writeln!(w, "    subgraph cluster_legend_sub {{")?;
        writeln!(w, "      x2 [label=\"Concurrent\"];")?;
        writeln!(w, "      x3 [label=\"Send\", color=green];")?;
        writeln!(w, "      style=dashed;")?;
        writeln!(w, "    }}")?;
        writeln!(w, "    x4 [shape=point,xlabel=\"End\"];")?;
        writeln!(w)?;
        writeln!(w, "    x0 -> x1 [style=dashed]")?;
        writeln!(
            w,
            "    x1 -> x2 [color=blue, label=\"is\\nbefore\", constraint=false]"
        )?;
        writeln!(w, "    x2 -> x3 [label=\"critical\\ndep.\"]")?;
        writeln!(
            w,
            "    x3 -> x2 [color=red, label=\"is\\nafter\", constraint=false]"
        )?;
        writeln!(
            w,
            "    x1 -> x2 [style=dashed, label=\"implicit\\ndep.\",lhead=cluster_legend_sub]"
        )?;
        writeln!(w, "    x3 -> x4 [style=dashed]")?;
        writeln!(w, "    label=\"Legend\"")?;
        writeln!(w, "  }}")?;
        // end
        writeln!(w, "}}")?;
        Ok(())
    }
    */
}
