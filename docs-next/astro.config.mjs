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
				'A serial terminal for network gear — console sessions for switches, routers, and firewalls.',
			logo: {
				src: './src/assets/icon.svg',
				replacesTitle: false,
			},
			favicon: '/favicon.svg',
			customCss: ['./src/styles/theme.css'],
			components: {
				Hero: './src/components/Hero.astro',
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
