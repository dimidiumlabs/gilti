// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod code_block;
pub mod diff;
pub mod document;
pub mod key_value;
pub mod layout;
pub mod log_table;
pub mod refs_table;
pub mod relative_time;
pub mod table;

pub fn file_mode(mode: u32) -> String {
    let mut value = String::with_capacity(10);
    value.push(match mode {
        0o040000 => 'd',
        0o120000 => 'l',
        0o160000 => 'm',
        _ => '-',
    });
    for bit in [
        0o400, 0o200, 0o100, 0o040, 0o020, 0o010, 0o004, 0o002, 0o001,
    ] {
        value.push(if mode & bit == 0 {
            '-'
        } else {
            match bit {
                0o400 | 0o040 | 0o004 => 'r',
                0o200 | 0o020 | 0o002 => 'w',
                _ => 'x',
            }
        });
    }
    value
}
