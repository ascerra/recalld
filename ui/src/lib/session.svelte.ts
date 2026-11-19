// Daemon connection state for the shell's banner and status line.
import { get, post } from './api/client';
import type { UiSession } from './api/types';

class Session {
  value = $state<UiSession | null>(null);
  loading = $state(true);
  starting = $state(false);

  get status() {
    return this.value?.daemon.status ?? (this.loading ? 'loading' : 'unavailable');
  }

  async refresh() {
    try {
      this.value = await get<UiSession>('/session');
    } catch {
      this.value = null;
    } finally {
      this.loading = false;
    }
  }

  async startDaemon() {
    this.starting = true;
    try {
      await post('/daemon/start');
    } catch {
      /* the banner stays; refresh shows the latest message */
    } finally {
      this.starting = false;
      await this.refresh();
    }
  }
}
export const session = new Session();
