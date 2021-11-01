use std::path::PathBuf;

use differential_datalog::DDlogDynamic;

use differential_datalog::{
    ddval::DDValConvert,
    program::{RelId, Update},
    DDlog,
};
use eyre::{eyre, Context};
use internment::{intern, ival};
use polonius_dl_ddlog::Relations;
use types::*;

use crate::fact_parser::{collect_facts, parse_facts};

#[derive(Debug)]
pub struct InvalidAccess {
    accessed_origin: String,
    node: String,
}

pub fn run(dir_name: &str) -> eyre::Result<Vec<InvalidAccess>> {
    // Parse out facts
    let manifest_dir = PathBuf::from(".");
    let path = manifest_dir.join(&dir_name);
    let input_path = path.join("program.txt");
    let data = std::fs::read_to_string(input_path)?;
    let program = parse_facts(data.as_str()).wrap_err("failed to parse input")?;
    let facts = collect_facts(&program)?;

    // Init ddlog
    let (hddlog, _init_state) = polonius_dl_ddlog::run(6, false).map_err(|e| eyre!(e))?;

    // Feed input data
    hddlog.transaction_start().map_err(|e| eyre!(e))?;
    let mut updates = Vec::new();
    for (fact_name, instances) in facts {
        let relation = match fact_name.as_str() {
            "mark_as_loan_origin" => Relations::MarkAsLoanOrigin,
            "access_origin" => Relations::AccessOrigin,
            "cfg_edge" => Relations::CfgEdge,
            "clear_origin" => Relations::ClearOrigin,
            "introduce_subset" => Relations::IntroduceSubset,
            "invalidate_origin" => Relations::InvalidateOrigin,
            _ => continue, // skip node text
        };
        for params in instances {
            let mut params = params.into_iter();
            let v = match relation {
                Relations::AccessOrigin => {
                    let origin = params
                        .next()
                        .ok_or(eyre!("missing origin of AccessOrigin"))?;
                    let node = params.next().ok_or(eyre!("missing node of AccessOrigin"))?;
                    AccessOrigin {
                        o: intern(origin),
                        n: intern(node),
                    }
                    .into_ddvalue()
                }
                Relations::CfgEdge => {
                    let node1 = params.next().ok_or(eyre!("missing node1 of CfgEdge"))?;
                    let node2 = params.next().ok_or(eyre!("missing node2 of CfgEdge"))?;
                    CfgEdge {
                        n1: intern(node1),
                        n2: intern(node2),
                    }
                    .into_ddvalue()
                }
                Relations::ClearOrigin => {
                    let origin = params
                        .next()
                        .ok_or(eyre!("missing origin of ClearOrigin"))?;
                    let node = params.next().ok_or(eyre!("missing node of ClearOrigin"))?;
                    ClearOrigin {
                        o: intern(origin),
                        n: intern(node),
                    }
                    .into_ddvalue()
                }
                Relations::IntroduceSubset => {
                    let origin1 = params
                        .next()
                        .ok_or(eyre!("missing origin1 of IntroduceSubset"))?;
                    let origin2 = params
                        .next()
                        .ok_or(eyre!("missing origin2 of IntroduceSubset"))?;
                    let node = params
                        .next()
                        .ok_or(eyre!("missing node of IntroduceSubset"))?;

                    IntroduceSubset {
                        o1: intern(origin1),
                        o2: intern(origin2),
                        n: intern(node),
                    }
                    .into_ddvalue()
                }
                Relations::InvalidateOrigin => {
                    let origin = params
                        .next()
                        .ok_or(eyre!("missing origin of InvalidateOrigin"))?;
                    let node = params
                        .next()
                        .ok_or(eyre!("missing node of InvalidateOrigin"))?;

                    InvalidateOrigin {
                        o: intern(origin),
                        n: intern(node),
                    }
                    .into_ddvalue()
                }
                Relations::MarkAsLoanOrigin => {
                    let origin = params
                        .next()
                        .ok_or(eyre!("missing origin of MarkAsLoanOrigin"))?;
                    MarkAsLoanOrigin { o: intern(origin) }.into_ddvalue()
                }
                _ => unreachable!("is output"),
            };
            updates.push(Update::Insert {
                relid: relation as RelId,
                v,
            });
        }
    }

    // Run ddlog
    hddlog
        .apply_updates(&mut updates.into_iter())
        .map_err(|e| eyre!(e))?;
    let mut delta = hddlog
        .transaction_commit_dump_changes()
        .map_err(|e| eyre!(e))?;

    // Process Output
    dbg!(&delta);
    let invalid_accesses = delta.get_rel(Relations::InvalidatedOriginAccessed as RelId);
    let invalid_accesses: Vec<_> = invalid_accesses
        .iter()
        .map(|(val, &weight)| {
            // 1 means insert, -1 means remove
            debug_assert_eq!(weight, 1);
            let access = InvalidatedOriginAccessed::from_ddvalue_ref(val);
            InvalidAccess {
                accessed_origin: ival(&access.o).clone(),
                node: ival(&access.n).clone(),
            }
        })
        .collect();
    dbg!(&invalid_accesses);

    hddlog.stop().map_err(|e| eyre!(e))?;
    Ok(invalid_accesses)
}
