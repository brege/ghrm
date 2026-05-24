import { html, LitElement, nothing } from 'lit';

interface ArchiveStatus {
  state: 'pending' | 'running' | 'complete' | 'failed';
  filename?: string;
  error?: string;
  done_files?: number;
  total_files?: number;
  done_bytes?: number;
  total_bytes?: number;
  doneFiles?: number;
  totalFiles?: number;
  doneBytes?: number;
  totalBytes?: number;
  percent?: number;
}

interface ArchiveJobResponse {
  download_url: string;
  status_url: string;
}

export class GhrmArchiveProgress extends LitElement {
  static properties = {
    status: { state: true },
    visible: { state: true },
  };

  private declare status: ArchiveStatus | null;
  private declare visible: boolean;

  private pollTimer: number | null = null;
  private hideTimer: number | null = null;

  constructor() {
    super();
    this.status = null;
    this.visible = false;
  }

  protected createRenderRoot(): HTMLElement {
    return this;
  }

  disconnectedCallback(): void {
    super.disconnectedCallback();
    this.clearTimers();
  }

  private clearTimers(): void {
    if (this.pollTimer !== null) {
      window.clearTimeout(this.pollTimer);
      this.pollTimer = null;
    }
    if (this.hideTimer !== null) {
      window.clearTimeout(this.hideTimer);
      this.hideTimer = null;
    }
  }

  async startJob(url: string): Promise<void> {
    this.clearTimers();
    this.status = {
      state: 'running',
      filename: 'archive',
      doneFiles: 0,
      totalFiles: 0,
      doneBytes: 0,
      totalBytes: 0,
      percent: 0,
    };
    this.visible = true;

    try {
      const response = await fetch(url, {
        method: 'POST',
        headers: { Accept: 'application/json' },
      });
      if (!response.ok) {
        throw new Error(`archive request failed: ${response.status}`);
      }
      const job = (await response.json()) as ArchiveJobResponse;
      this.triggerDownload(job.download_url);
      await this.poll(job.status_url);
    } catch {
      this.status = {
        state: 'failed',
        filename: 'archive',
        error: 'Archive failed',
        percent: 100,
      };
    }
  }

  private async poll(statusUrl: string): Promise<void> {
    try {
      const response = await fetch(statusUrl, {
        headers: { Accept: 'application/json' },
      });
      if (!response.ok) {
        throw new Error(`archive status failed: ${response.status}`);
      }
      const status = (await response.json()) as ArchiveStatus;
      this.status = status;

      if (status.state === 'complete') {
        this.hideTimer = window.setTimeout(() => this.hide(), 1800);
        return;
      }
      if (status.state === 'failed') {
        return;
      }
      this.pollTimer = window.setTimeout(() => this.poll(statusUrl), 500);
    } catch {
      this.status = {
        state: 'failed',
        filename: 'archive',
        error: 'Archive failed',
        percent: 100,
      };
    }
  }

  private hide(): void {
    this.visible = false;
    this.clearTimers();
  }

  private triggerDownload(url: string): void {
    const link = document.createElement('a');
    link.href = url;
    link.download = '';
    link.dataset.ghrmNative = '1';
    link.hidden = true;
    document.body.append(link);
    link.click();
    link.remove();
  }

  private get percent(): number {
    return Math.max(0, Math.min(100, this.status?.percent || 0));
  }

  private get label(): string {
    if (!this.status) return 'Building archive';
    if (this.status.state === 'pending') {
      return `Starting ${this.status.filename || 'archive'}`;
    }
    if (this.status.state === 'complete') {
      return 'Archive complete';
    }
    if (this.status.state === 'failed') {
      return this.status.error || 'Archive failed';
    }
    return `Downloading ${this.status.filename || 'archive'}`;
  }

  private get count(): string {
    if (!this.status) return '';
    const parts = [`${this.percent}%`];
    const files = this.fileCount;
    const bytes = this.byteCount;
    if (files) parts.push(files);
    if (bytes) parts.push(bytes);
    return parts.join(' · ');
  }

  private get fileCount(): string {
    if (!this.status) return '';
    const done = Number(this.status.done_files ?? this.status.doneFiles ?? 0);
    const total = Number(
      this.status.total_files ?? this.status.totalFiles ?? 0,
    );
    if (!total) return '';
    return `${done} / ${total} files`;
  }

  private get byteCount(): string {
    if (!this.status) return '';
    const done = Number(this.status.done_bytes ?? this.status.doneBytes ?? 0);
    const total = Number(
      this.status.total_bytes ?? this.status.totalBytes ?? 0,
    );
    if (!total) return '';
    return `${this.formatBytes(done)} / ${this.formatBytes(total)}`;
  }

  private formatBytes(value: number): string {
    if (value < 1024) return `${value} B`;
    const units = ['KB', 'MB', 'GB', 'TB'];
    let size = value / 1024;
    for (const unit of units) {
      if (size < 1024) {
        return `${size.toFixed(size < 10 ? 1 : 0)} ${unit}`;
      }
      size /= 1024;
    }
    return `${size.toFixed(0)} PB`;
  }

  protected render() {
    if (!this.visible) {
      return nothing;
    }

    return html`
      <div
        class="ghrm-archive-progress"
        data-state=${this.status?.state || 'running'}
        aria-live="polite"
      >
        <div class="ghrm-archive-progress-row">
          <span class="ghrm-archive-progress-label">${this.label}</span>
          <span class="ghrm-archive-progress-count">${this.count}</span>
        </div>
        <div class="ghrm-archive-progress-track">
          <div
            class="ghrm-archive-progress-fill"
            style="width: ${this.percent}%"
          ></div>
        </div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'ghrm-archive-progress': GhrmArchiveProgress;
  }
}

customElements.define('ghrm-archive-progress', GhrmArchiveProgress);
