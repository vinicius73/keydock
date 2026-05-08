// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://docs.astro.build/en/guides/deploy/github/
// CI sets ASTRO_SITE / ASTRO_BASE (see .github/workflows/deploy-site.yml).
const site = process.env.ASTRO_SITE?.trim() || undefined;
const astroBase = process.env.ASTRO_BASE?.trim();
const base = astroBase === undefined || astroBase === '' ? undefined : astroBase;

// https://astro.build/config
export default defineConfig({
	...(site ? { site } : {}),
	...(base !== undefined ? { base } : {}),
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
