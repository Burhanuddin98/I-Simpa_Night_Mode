// The Help menu's model (parity A20): what each item is called and says, and the topic the core
// opens for it (src-tauri/src/help.rs). The web addresses live in the core, never here: the UI's
// bundle carries no http(s) address outside the m9 gate's listed exceptions.
//
// Pure: MenuBar.tsx draws it; helpModel.test.ts holds every topic to help.rs's list.

/** A topic `help_open` takes (help.rs). */
export type HelpTopic = 'upstream-guide' | 'upstream-site' | 'source';

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
  const l = HELP_LINKS.find((x) => x.topic === topic);
  if (!l) throw new Error(`no Help topic ${topic}`);
  return l;
}
