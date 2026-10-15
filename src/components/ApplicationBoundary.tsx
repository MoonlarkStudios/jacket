import { Component, type ReactNode } from 'react';

export class ApplicationBoundary extends Component<
  { children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  render() {
    if (this.state.failed) {
      return (
        <main className="startup-recovery" role="alert">
          <p>Jacket couldn’t open. Please reload to try again.</p>
          <button type="button" onClick={() => window.location.reload()}>
            Reload Jacket
          </button>
        </main>
      );
    }
    return this.props.children;
  }
}
