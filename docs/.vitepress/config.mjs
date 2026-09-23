import { defineConfig } from 'vitepress'

const repo = 'https://github.com/e-choness/pdf-sanitizer'

export default defineConfig({
  title: 'PDF Sanitizer',
  description:
    'Offline desktop app that strips scripts, attachments, links and metadata from PDF files.',
  lang: 'en-US',
  // Served from https://e-choness.github.io/pdf-sanitizer/
  base: '/pdf-sanitizer/',
  cleanUrls: true,
  lastUpdated: true,

  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/pdf-sanitizer/logo.svg' }],
    ['meta', { name: 'theme-color', content: '#2563eb' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:title', content: 'PDF Sanitizer' }],
    [
      'meta',
      {
        property: 'og:description',
        content: 'Strip scripts, attachments, links and metadata from PDFs — fully offline.',
      },
    ],
  ],

  themeConfig: {
    logo: '/logo.svg',

    nav: [
      { text: 'Guide', link: '/guide/getting-started', activeMatch: '/guide/' },
      { text: 'Development', link: '/development/setup', activeMatch: '/development/' },
      { text: 'Changelog', link: '/changelog' },
      { text: 'Download', link: `${repo}/releases/latest` },
    ],

    sidebar: {
      '/guide/': [
        {
          text: 'User guide',
          items: [
            { text: 'Getting started', link: '/guide/getting-started' },
            { text: 'Using the app', link: '/guide/usage' },
            { text: 'What gets removed', link: '/guide/sanitization' },
            { text: 'Settings', link: '/guide/settings' },
            { text: 'Troubleshooting', link: '/guide/troubleshooting' },
          ],
        },
      ],
      '/development/': [
        {
          text: 'Development',
          items: [
            { text: 'Local setup', link: '/development/setup' },
            { text: 'Architecture', link: '/development/architecture' },
            { text: 'Testing', link: '/development/testing' },
            { text: 'Building', link: '/development/building' },
            { text: 'Releasing', link: '/development/releasing' },
          ],
        },
      ],
    },

    socialLinks: [{ icon: 'github', link: repo }],

    editLink: {
      pattern: `${repo}/edit/main/docs/:path`,
      text: 'Edit this page on GitHub',
    },

    search: { provider: 'local' },

    footer: {
      message: 'Released under the PolyForm Noncommercial License 1.0.0.',
      copyright: 'Copyright © 2026 Beili (Echo) Yin',
    },
  },
})
