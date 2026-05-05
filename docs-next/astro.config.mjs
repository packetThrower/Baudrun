// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

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
			logo: {
				src: './src/assets/icon.svg',
				replacesTitle: false,
			},
			favicon: '/favicon.svg',
			customCss: ['./src/styles/theme.css'],

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
				Hero: './src/components/Hero.astro',
				// Wraps Starlight's default SocialIcons to add a "Docs"
				// quick-access pill linking to /install/ — the most
				// common entry point for visitors landing on a deep
				// page who want to start over.
				SocialIcons: './src/components/SocialIcons.astro',
			},
			social: [
				{
					icon: 'github',
					label: 'GitHub',
					href: 'https://github.com/packetThrower/Baudrun',
				},
			],
			editLink: {
				baseUrl: 'https://github.com/packetThrower/Baudrun/edit/main/docs-next/',
			},
			sidebar: [
				{ label: 'Install', slug: 'install' },
				{
					label: 'Usage',
					items: [
						{ label: 'Profiles', slug: 'usage/profiles' },
						{ label: 'Keyboard shortcuts', slug: 'usage/shortcuts' },
						{ label: 'Advanced settings', slug: 'usage/advanced' },
						{ label: 'USB-serial adapters', slug: 'usage/adapters' },
						{ label: 'Screenshots', slug: 'usage/screenshots' },
					],
				},
				{
					label: 'Authoring',
					items: [
						{ label: 'Terminal themes', slug: 'authoring/themes' },
						{ label: 'App skins', slug: 'authoring/skins' },
						{ label: 'Syntax highlighting', slug: 'authoring/highlighting' },
						// Standalone HTML page in public/. The `attrs` field
						// gives it the same external-link affordance as the
						// GitHub social link.
						{
							label: 'Rule playground',
							link: '/Baudrun/playground.html',
							attrs: { target: '_blank', rel: 'noopener' },
						},
					],
				},
				{
					label: 'Reference',
					items: [
						{ label: 'Accessibility', slug: 'reference/accessibility' },
						{ label: 'Requirements', slug: 'reference/requirements' },
					],
				},
				{ label: 'Changelog', slug: 'changelog' },
			],
			lastUpdated: true,
		}),
	],
});
