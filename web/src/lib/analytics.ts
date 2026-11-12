import type { RequestTotals, UsedModel } from './api';

const DAY = 86400;
const SERIES = 6;

export const OTHER = 'var(--series-other)';

const compactFormat = new Intl.NumberFormat('en', {
	notation: 'compact',
	maximumFractionDigits: 1
});
const wholeFormat = new Intl.NumberFormat('en');
const relativeFormat = new Intl.RelativeTimeFormat('en', { numeric: 'auto', style: 'narrow' });

export function utcOffset(): number {
	return -new Date().getTimezoneOffset() * 60;
}

export function today(): number {
	return Math.floor((Date.now() / 1000 + utcOffset()) / DAY);
}

export function startOf(day: number): number {
	return day * DAY - utcOffset();
}

export function dayOf(time: number): number {
	return Math.floor((time + utcOffset()) / DAY);
}

export function dateOf(day: number, options: Intl.DateTimeFormatOptions): string {
	return new Date(day * DAY * 1000).toLocaleDateString('en', { ...options, timeZone: 'UTC' });
}

export function modelId({ provider, model }: UsedModel): string {
	return `${provider}/${model}`;
}

export function seriesOf(ranking: UsedModel[], model: UsedModel): number | null {
	const rank = ranking.findIndex((ranked) => modelId(ranked) === modelId(model));
	return rank >= 0 && rank < SERIES ? rank : null;
}

export function seriesColor(series: number | null): string {
	return series === null ? OTHER : `var(--series-${series + 1})`;
}

export function tokens(totals: RequestTotals): number {
	return totals.input + totals.output;
}

export function compact(value: number): string {
	return compactFormat.format(value);
}

export function whole(value: number): string {
	return wholeFormat.format(Math.round(value));
}

export function percent(part: number, total: number): string {
	if (total <= 0) return '0%';
	const share = (part / total) * 100;
	return `${share > 0 && share < 10 ? share.toFixed(1) : Math.round(share)}%`;
}

export function duration(ms: number): string {
	if (ms < 1000) return `${Math.round(ms)}ms`;
	const seconds = ms / 1000;
	if (seconds < 60) return `${seconds < 10 ? seconds.toFixed(1) : Math.round(seconds)}s`;
	const minutes = Math.floor(seconds / 60);
	return `${minutes}m ${Math.round(seconds - minutes * 60)}s`;
}

export function speed(totals: RequestTotals): number | null {
	return totals.duration_ms > 0 ? totals.output / (totals.duration_ms / 1000) : null;
}

export function ago(time: number): string {
	const seconds = time - Date.now() / 1000;
	const units: [Intl.RelativeTimeFormatUnit, number][] = [
		['day', DAY],
		['hour', 3600],
		['minute', 60]
	];
	for (const [unit, size] of units) {
		if (Math.abs(seconds) >= size)
			return relativeFormat.format(Math.round(seconds / size), unit);
	}
	return 'just now';
}
