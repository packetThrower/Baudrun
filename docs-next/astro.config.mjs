// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import sitemap from '@astrojs/sitemap';

// https://astro.build/config
export default defineConfig({
	site: 'https://packetthrower.github.io',
	base: '/Baudrun/',
	trailingSlash: 'ignore',
	integrations: [
		starlight({
			title: 'Baudrun',
			description:
				'A serial terminal for network gear. Console sessions for switches, routers, and firewalls.',
			favicon: '/favicon.svg',
			customCss: ['@packetthrower/site-theme/styles/starlight.css'],

			// Site-wide head additions for discoverability:
			//
			// 1. Open Graph / Twitter image. Without this, link-preview
			//    unfurlers (Slack, Discord, Twitter, iMessage, etc.)
			//    render every share as a bare text card.
			// 2. Google Search Console verification slot. Replace the
			//    `content` value with the token Search Console gives
			//    you after you claim the property at
			//    https://search.google.com/search-console — once
			//    verified, you can submit
			//    https://packetthrower.github.io/Baudrun/sitemap-index.xml
			//    and see crawl + search-performance reports.
			head: [
				{
					tag: 'meta',
					attrs: {
						property: 'og:image',
						content: 'https://packetthrower.github.io/Baudrun/og-image.png',
					},
				},
				{
					tag: 'meta',
					attrs: {
						property: 'og:image:width',
						content: '1200',
					},
				},
				{
					tag: 'meta',
					attrs: {
						property: 'og:image:height',
						content: '630',
					},
				},
				{
					tag: 'meta',
					attrs: {
						name: 'twitter:image',
						content: 'https://packetthrower.github.io/Baudrun/og-image.png',
					},
				},
				{
					tag: 'meta',
					attrs: {
						name: 'google-site-verification',
						// Issued by Search Console for
						// https://packetthrower.github.io/Baudrun/. Don't
						// remove it: Search Console re-validates this tag
						// periodically and the property loses verified
						// status if the tag disappears.
						content: 'tCAEO_FaKHi5IgbdZ83ZbNYJ4orBbsCLDYFTO1tjqbg',
					},
				},
			],
			components: {
				// Shared packetThrower header: global bar (Apps menu, search,
				// theme, GitHub) + Baudrun's app bar. The landing page at
				// /Baudrun/ is src/pages/index.astro, outside Starlight.
				Header: '@packetthrower/site-theme/components/StarlightHeader.astro',
			},
			editLink: {
				baseUrl: 'https://github.com/packetThrower/Baudrun/edit/main/docs-next/',
			},
			sidebar: [
				// Same groups, in the same order, on every packetThrower docs site.
				{
					label: 'Get started',
					items: [
						{ label: 'Install', slug: 'install' },
						{ label: 'Requirements', slug: 'reference/requirements' },
					],
				},
				{
					label: 'Guides',
					items: [
						{ label: 'Profiles', slug: 'usage/profiles' },
						{ label: 'Keyboard shortcuts', slug: 'usage/shortcuts' },
						{ label: 'Advanced settings', slug: 'usage/advanced' },
						{ label: 'USB-serial adapters', slug: 'usage/adapters' },
					],
				},
				{
					label: 'Customize',
					items: [
						{ label: 'Terminal themes', slug: 'authoring/themes' },
						{ label: 'App skins', slug: 'authoring/skins' },
						{ label: 'Syntax highlighting', slug: 'authoring/highlighting' },
						{ label: 'Translations', slug: 'authoring/translations' },
						// Standalone HTML page in public/. The `attrs` field
						// gives it the same external-link affordance as the
						// GitHub social link.
						{
							label: 'Rule playground',
							// Absolute URL on purpose. Starlight's sidebar
							// `link` field, when given a site-relative path,
							// (a) prepends Astro's configured `base` (so
							// authors who hand-write `/Baudrun/...` get a
							// double prefix — `/Baudrun/Baudrun/...`), and
							// (b) strips trailing `.html` to match its own
							// pretty-URL routing convention — together
							// producing `/Baudrun/Baudrun/playground` (404)
							// from `/Baudrun/playground.html`. An absolute
							// `https://...` URL is treated as external and
							// passes through unmodified. The local-dev
							// "downside" is that clicking this in `astro dev`
							// goes to the live deploy rather than the local
							// public/playground.html — fine for sidebar nav.
							link: 'https://packetthrower.github.io/Baudrun/playground.html',
							attrs: { target: '_blank', rel: 'noopener' },
						},
					],
				},
				{
					label: 'Reference',
					items: [
						{ label: 'Screenshots', slug: 'usage/screenshots' },
						{ label: 'Accessibility', slug: 'reference/accessibility' },
						{ label: 'Testing', slug: 'reference/testing' },
					],
				},
				{
					label: 'Project',
					items: [
						{ label: 'Changelog', slug: 'changelog' },
						{
							label: 'Issues',
							link: 'https://github.com/packetThrower/Baudrun/issues',
							attrs: { target: '_blank', rel: 'noopener' },
						},
					],
				},
			],
			lastUpdated: true,
		}),
		// Explicit `@astrojs/sitemap` config — Starlight auto-pulls
		// the integration, but its default emits `<loc>`-only
		// entries. Adding it here lets us pass a `lastmod` so each
		// URL carries a freshness timestamp. Google Search Console
		// uses lastmod for crawl scheduling; without it, every
		// entry looks equally stale and the crawler is less
		// aggressive about re-indexing changed pages.
		//
		// `new Date()` evaluates at build time, so every page in
		// the sitemap gets the deployment timestamp. Per-page
		// per-file mtime would be more accurate but requires a
		// `serialize()` callback that walks git history per URL
		// — not worth the build-time cost for a docs site this
		// small, where everything tends to ship in the same
		// release anyway.
		sitemap({
			lastmod: new Date(),
		}),
	],
});
