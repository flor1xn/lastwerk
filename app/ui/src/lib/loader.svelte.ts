// Lädt Daten asynchron und verwirft veraltete Antworten: Kommt eine ältere Anfrage nach einer
// neueren an (schnelles Blättern), überschreibt sie das Ergebnis nicht.

export class Loader<T> {
  data = $state<T | null>(null);
  loading = $state(false);
  error = $state<unknown>(null);
  #seq = 0;

  async run(fetcher: () => Promise<T>): Promise<void> {
    const mine = ++this.#seq;
    this.loading = true;
    try {
      const result = await fetcher();
      if (mine === this.#seq) {
        this.data = result;
        this.error = null;
      }
    } catch (e) {
      if (mine === this.#seq) {
        this.error = e;
        this.data = null;
      }
    } finally {
      if (mine === this.#seq) this.loading = false;
    }
  }
}
