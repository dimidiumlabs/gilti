// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Period {
    Week,
    Month,
    Quarter,
    Year,
}

impl Period {
    pub fn name(self) -> &'static str {
        match self {
            Self::Week => "week",
            Self::Month => "month",
            Self::Quarter => "quarter",
            Self::Year => "year",
        }
    }
}

pub struct Stats {
    pub repository: super::repository::Info,
    pub labels: Vec<String>,
    pub authors: Vec<Author>,
    pub totals: Vec<usize>,
}

pub struct Author {
    pub name: String,
    pub counts: Vec<usize>,
    pub total: usize,
}

impl Stats {
    pub fn load(root: &Path, name: &str, period: Period) -> Result<Self, super::Error> {
        let repository = super::repository::open(root, name)?;
        let info = super::repository::info(&repository, name);
        let head = super::revision::commit(&repository, &crate::router::Revision::Head)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs() as i64);
        let labels = labels(period, now);
        let positions = labels
            .iter()
            .enumerate()
            .map(|(index, label)| (label.clone(), index))
            .collect::<BTreeMap<_, _>>();
        let mut authors = BTreeMap::<String, Vec<usize>>::new();
        let mut walk = repository.revwalk().map_err(super::Error::from_git)?;
        walk.push(head.id()).map_err(super::Error::from_git)?;
        walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)
            .map_err(super::Error::from_git)?;
        for oid in walk {
            let commit = repository
                .find_commit(oid.map_err(super::Error::from_git)?)
                .map_err(super::Error::from_git)?;
            if commit.parent_count() > 1 {
                continue;
            }
            let label = label(period, commit.time().seconds());
            let Some(index) = positions.get(&label).copied() else {
                continue;
            };
            let signature = commit.author();
            let author = signature
                .name()
                .map(str::to_owned)
                .unwrap_or_else(|_| String::from_utf8_lossy(signature.name_bytes()).into_owned());
            authors.entry(author).or_insert_with(|| vec![0; 4])[index] += 1;
        }
        let mut authors = authors
            .into_iter()
            .map(|(name, counts)| Author {
                total: counts.iter().sum(),
                name,
                counts,
            })
            .collect::<Vec<_>>();
        authors.sort_by(|left, right| {
            right
                .total
                .cmp(&left.total)
                .then_with(|| left.name.cmp(&right.name))
        });
        let totals = (0..4)
            .map(|index| authors.iter().map(|author| author.counts[index]).sum())
            .collect();
        Ok(Self {
            repository: info,
            labels,
            authors,
            totals,
        })
    }
}

fn labels(period: Period, now: i64) -> Vec<String> {
    let mut time = tm(now);
    truncate(period, &mut time);
    let mut labels = Vec::with_capacity(4);
    for _ in 0..4 {
        labels.push(pretty(period, &time));
        decrement(period, &mut time);
    }
    labels.reverse();
    labels
}

fn label(period: Period, timestamp: i64) -> String {
    let mut time = tm(timestamp);
    truncate(period, &mut time);
    pretty(period, &time)
}

fn tm(timestamp: i64) -> libc::tm {
    let mut result = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: both pointers are valid for the duration of the call.
    unsafe {
        libc::gmtime_r(&timestamp, result.as_mut_ptr());
        result.assume_init()
    }
}

fn normalize(time: &mut libc::tm) {
    // SAFETY: time points to an initialized libc::tm and timegm normalizes it in place.
    unsafe { libc::timegm(time) };
}

fn truncate(period: Period, time: &mut libc::tm) {
    match period {
        Period::Week => {
            time.tm_mday -= (time.tm_wday + 6) % 7;
        }
        Period::Month => time.tm_mday = 1,
        Period::Quarter => {
            time.tm_mday = 1;
            time.tm_mon -= time.tm_mon % 3;
        }
        Period::Year => {
            time.tm_mday = 1;
            time.tm_mon = 0;
        }
    }
    normalize(time);
}

fn decrement(period: Period, time: &mut libc::tm) {
    match period {
        Period::Week => time.tm_mday -= 7,
        Period::Month => time.tm_mon -= 1,
        Period::Quarter => time.tm_mon -= 3,
        Period::Year => time.tm_year -= 1,
    }
    normalize(time);
}

fn pretty(period: Period, time: &libc::tm) -> String {
    match period {
        Period::Week => strftime("W%V %G", time),
        Period::Month => {
            const MONTHS: [&str; 12] = [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ];
            format!("{} {}", MONTHS[time.tm_mon as usize], time.tm_year + 1900)
        }
        Period::Quarter => format!("Q{} {}", time.tm_mon / 3 + 1, time.tm_year + 1900),
        Period::Year => (time.tm_year + 1900).to_string(),
    }
}

fn strftime(format: &str, time: &libc::tm) -> String {
    let format = std::ffi::CString::new(format).expect("static format has no NUL");
    let mut buffer = [0_u8; 32];
    // SAFETY: the output buffer, format string and tm pointers are valid.
    let length = unsafe {
        libc::strftime(
            buffer.as_mut_ptr().cast::<libc::c_char>(),
            buffer.len(),
            format.as_ptr(),
            time,
        )
    };
    std::str::from_utf8(&buffer[..length])
        .expect("strftime emits ASCII for this format")
        .to_owned()
}
