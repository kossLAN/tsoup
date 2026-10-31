export const verbs = [
	'Thinking',
	'Working',
	'Mulling',
	'Tinkering',
	'Puzzling',
	'Untangling',
	'Considering',
	'Figuring',
	'Digging',
	'Sorting',
	'Simmering',
	'Stirring',
	'Steeping'
];

export const prompts = [
	'What are we working on?',
	'What do you want to build?',
	'Where should we start?',
	'What needs fixing?',
	"What's next?",
	'What can I help with?'
];

export function pick<T>(items: readonly T[], not?: T): T {
	const pool = items.length > 1 ? items.filter((item) => item !== not) : items;
	return pool[Math.floor(Math.random() * pool.length)];
}
