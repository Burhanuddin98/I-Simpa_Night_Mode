// The Console (design:175-275, the Console tab): every line with its class as a text label.
// Hand-over stub from the M10 foundation (M9's pane); the scene package owns it from here.
import { consoleStore, useStore } from '../store';

export function ConsolePane() {
  const lines = useStore(consoleStore);
  return (
    <div className="console" role="log" aria-live="polite">
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
