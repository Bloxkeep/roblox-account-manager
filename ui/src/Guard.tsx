// A part of the page that fails to draw (a bug, a state it didn't expect) goes blank on its own instead of taking the
// whole page with it: React would otherwise take down everything drawn in the same root (the sidebar, Lock…). It tries
// again with the app's next state, and says what went wrong in the console.
import { Component, type ReactNode } from 'react';
import { useAppState } from './host';

class Guard extends Component<{ name: string; retry: unknown; children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  componentDidCatch(error: unknown) { console.error(`Bloxkeep: ${this.props.name} stopped drawing.`, error); }
  componentDidUpdate(prev: { retry: unknown }) { if (this.state.failed && prev.retry !== this.props.retry) this.setState({ failed: false }); }
  render() { return this.state.failed ? null : this.props.children; }
}

export function Guarded({ name, children }: { name: string; children: ReactNode }) {
  return <Guard name={name} retry={useAppState()}>{children}</Guard>;
}
