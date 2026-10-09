// The Help menu's model (parity A20): what each item is called and says, and the topic the core
// opens for it (src-tauri/src/help.rs). The web addresses live in the core, never here: the UI's
// bundle carries no http(s) address outside the m9 gate's listed exceptions.
//
// Pure: MenuBar.tsx draws it; helpModel.test.ts holds every topic to help.rs's list.

/** A topic `help_open` takes (help.rs): a web page (`LINKS`) or a page shipped in the app (`PAGES`). */
export type HelpTopic = 'manual' | 'licence' | 'notices' | 'tutorial-1' | 'tutorial-2' | 'tutorial-3' | 'upstream-guide' | 'upstream-site' | 'source';

export interface HelpLink {
  topic: HelpTopic;
  /** The menu item's id (`data-menu-item`). */
  id: string;
  label: string;
  /** The item's tooltip: where it goes and what it is for. */
  title: string;
  /** What the Console says was opened: `Opened <what> in your browser`. */
  what: string;
}

/** A22: the user manual, shipped inside app.exe (help.rs `PAGES`), the Help menu's first item. */
export const HELP_MANUAL: HelpLink = {
  topic: 'manual',
  id: 'help-manual',
  label: 'User manual',
  title: 'This version’s user manual, in your browser. It ships with the app and needs no network',
  what: 'the user manual',
};

/** A23: the Help menu's last item, Help › About (AboutDialog.tsx). */
export const ABOUT_LABEL = 'About Night Mode';

/** A23: the GPL-3.0's text, opened from About. */
export const HELP_LICENCE: HelpLink = {
  topic: 'licence',
  id: 'about-licence-text',
  label: 'GPL-3.0 licence',
  title: 'The GNU General Public License, version 3, in full',
  what: 'the GPL-3.0 licence',
};

/** A23: THIRD-PARTY-NOTICES.txt, opened from About. */
export const HELP_NOTICES: HelpLink = {
  topic: 'notices',
  id: 'about-notices',
  label: 'Third-party notices',
  title: 'Every library, solver, font and data set the app carries, with its licence text',
  what: 'the third-party notices',
};

/** A43: upstream's tutorials. Each menu item opens the tutorial's project (examples.rs, `example`)
 * as the user's copy and its text page (help.rs PAGES, `topic`). */
export interface Tutorial extends HelpLink {
  example: string;
}

export const TUTORIALS: readonly Tutorial[] = [1, 2, 3].map((n) => {
  const room = ['a teaching room', 'the Elmia hall', 'an industrial hall'][n - 1];
  return {
    topic: `tutorial-${n}` as HelpTopic,
    id: `help-tutorial-${n}`,
    example: `tutorial-${n}`,
    label: `Tutorial ${n}: ${room}`,
    title: `Upstream I-Simpa’s tutorial ${n}: opens its project as your own copy, and its text, with what upstream expects, in your browser`,
    what: `the text of tutorial ${n}`,
  };
});

/** The pages shipped in the app, in help.rs `PAGES`'s order. */
export const HELP_PAGES: readonly HelpLink[] = [HELP_MANUAL, HELP_LICENCE, HELP_NOTICES, ...TUTORIALS];

/** The web destinations, in the menu's order. */
export const HELP_LINKS: readonly HelpLink[] = [
  {
    topic: 'upstream-guide',
    id: 'help-upstream-guide',
    label: 'I-Simpa user guide (online)',
    title:
      'Upstream I-Simpa’s own user guide, in your browser. It is written for upstream’s interface, not this one; its chapters on the SPPS and TCR codes describe the solvers this app runs',
    what: 'upstream I-Simpa’s user guide',
  },
  {
    topic: 'upstream-site',
    id: 'help-upstream-site',
    label: 'I-Simpa website',
    title: 'Upstream I-Simpa’s website, in your browser: the project whose solvers this app runs',
    what: 'upstream I-Simpa’s website',
  },
  {
    topic: 'source',
    id: 'help-source',
    label: 'Night Mode source code',
    title: 'This app’s source code on GitHub, in your browser (GPL-3.0)',
    what: 'Night Mode’s source code',
  },
];

export function helpLink(topic: HelpTopic): HelpLink {
  const l = [...HELP_PAGES, ...HELP_LINKS].find((x) => x.topic === topic);
  if (!l) throw new Error(`no Help topic ${topic}`);
  return l;
}
