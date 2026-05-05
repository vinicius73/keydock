// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	integrations: [
		starlight({
			title: 'Keydock',
			description: 'Multi-tenant HTTP key-value service',
			social: [
				{
					icon: 'github',
					label: 'GitHub',
					href: 'https://github.com/vinicius73/keydock',
				},
			],
			sidebar: [
				{
					label: 'Guides',
					items: [
						{ label: 'Quick Start', slug: 'guides/quickstart' },
					],
				},
				{
					label: 'Reference',
					items: [
						{ label: 'HTTP API', slug: 'reference/http-api' },
						{ label: 'TypeScript SDK', slug: 'reference/sdk' },
					],
				},
			],
		}),
	],
});
