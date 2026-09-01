// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

pub fn utc(timestamp: i64) -> Option<libc::tm> {
    let mut value = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: both pointers are valid for the call; the result is read only when
    // gmtime_r reports success and has initialized it.
    let result = unsafe { libc::gmtime_r(&timestamp, value.as_mut_ptr()) };
    (!result.is_null()).then(|| unsafe { value.assume_init() })
}
