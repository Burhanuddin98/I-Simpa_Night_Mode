// The Console (design:247-257): every line with its class, FAIL, WARN, INFO or OK, as a text
// label (never colour alone). It follows new lines while it is scrolled to the bottom, and stays
// put while the user reads further up.
import { useLayoutEffect, useRef } from 'react';
import { consoleStore, useStore } from '../../store';

export function ConsolePane() {
  const lines = useStore(consoleStore);
  const root = useRef<HTMLDivElement>(null);
  const pinned = useRef(true);

  useLayoutEffect(() => {
    const scroller = root.current?.parentElement;
    if (!scroller) return;
    const onScroll = () => {
      pinned.current = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 24;
    };
    scroller.addEventListener('scroll', onScroll, { passive: true });
    return () => scroller.removeEventListener('scroll', onScroll);
  }, []);

  useLayoutEffect(() => {
    const scroller = root.current?.parentElement;
    if (scroller && pinned.current) scroller.scrollTop = scroller.scrollHeight;
  }, [lines]);

  return (
    <div className="console" role="log" aria-live="polite" ref={root}>
      {lines.length === 0 && <div className="console-empty empty">Nothing logged yet.</div>}
      {lines.map((l, i) => (
        <div key={i} className={`console-line ${l.tag}`}>
          <span className="time">{l.time}</span>
          <span className="tag">{l.tag}</span>
          <span className="text">{l.text}</span>
        </div>
      ))}
    </div>
  );
}
