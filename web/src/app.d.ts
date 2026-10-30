declare global {
	namespace App {}

	interface ImportMetaEnv {
		readonly VITE_BACKEND_PORT: string;
	}
}

export {};
