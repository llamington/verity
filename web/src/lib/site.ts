// The live site, for absolute URLs (link previews, search engines) and for turning on analytics
// there only. static/sitemap.xml and static/robots.txt repeat it.
export const SITE = 'https://verity.llamington.workers.dev';
// The source repository, linked from the landing page and its structured data.
export const REPO = 'https://github.com/llamington/verity';
// GitHub redirects these to the newest release, so they never need a version bump.
export const RELEASES = `${REPO}/releases/latest`;
export const DS_DOWNLOAD = `${RELEASES}/download/verity.nds`;
