// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: AGPL-3.0-or-later

const MIN = 60, HOUR = 60 * MIN, DAY = 24 * HOUR, YEAR = 365 * DAY;

type AgeUnit = "minutes" | "hours" | "days" | "weeks" | "months" | "years";

interface AgeRange {
    readonly unit: AgeUnit;
    readonly suffix: string;
    readonly size: number;
    readonly limit: number;
    readonly period: number;
}

const AGE_RANGES: readonly AgeRange[] = [
    { unit: "minutes", suffix: "min.", size: MIN, limit: 2 * HOUR, period: 10 },
    {
        unit: "hours",
        suffix: "hours",
        size: HOUR,
        limit: 2 * DAY,
        period: 5 * MIN,
    },
    {
        unit: "days",
        suffix: "days",
        size: DAY,
        limit: 14 * DAY,
        period: 30 * MIN,
    },
    {
        unit: "weeks",
        suffix: "weeks",
        size: 7 * DAY,
        limit: 60 * DAY,
        period: DAY,
    },
    {
        unit: "months",
        suffix: "months",
        size: 30 * DAY,
        limit: 730 * DAY,
        period: DAY,
    },
    {
        unit: "years",
        suffix: "years",
        size: YEAR,
        limit: Infinity,
        period: DAY,
    },
];

function rangeFor(age: number): AgeRange {
    return AGE_RANGES.find(({ limit }) => age < limit) ??
        AGE_RANGES[AGE_RANGES.length - 1]!;
}

function start(): void {
    const elements = document.querySelectorAll<HTMLElement>(
        "[data-relative-time]",
    );
    if (elements.length === 0) return;

    const now = Math.floor(Date.now() / 1000);
    let period = DAY;
    elements.forEach((el) => {
        const timestamp = Number(el.dataset.timestamp);
        if (Number.isFinite(timestamp)) {
            const age = Math.max(0, now - timestamp);
            period = Math.min(period, rangeFor(age).period);
        }
    });

    window.setInterval(() => {
        const now = Math.floor(Date.now() / 1000);

        for (const el of elements) {
            const timestamp = Number(el.dataset.timestamp);
            if (!Number.isFinite(timestamp)) {
                continue;
            }

            const age = Math.max(0, now - timestamp);
            const range = rangeFor(age);
            const text = `${Math.floor(age / range.size)} ${range.suffix} ago`;

            if (el.textContent !== text) {
                el.textContent = text;
            }

            if (el.dataset.unit !== range.unit) {
                el.dataset.unit = range.unit;
            }
        }
    }, period * 1000);
}

document.addEventListener("DOMContentLoaded", start, { once: true });
