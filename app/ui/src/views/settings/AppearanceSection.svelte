<script lang="ts">
  import { setLanguage, setThemeChoice } from '../../lib/appearance';
  import type { Locale, Theme } from '../../lib/backend/types';
  import { locale, LOCALES, t } from '../../lib/i18n';
  import { theme } from '../../lib/state';

  const themes: Theme[] = ['dark', 'light', 'system'];
</script>

<section class="section" data-testid="appearance-section">
  <h3>{$t('ui.appearance.title')}</h3>
  <div class="fields">
    <label>
      {$t('ui.appearance.language')}
      <select
        value={$locale}
        onchange={(e) => void setLanguage(e.currentTarget.value as Locale)}
        data-testid="language-select"
      >
        {#each LOCALES as l (l)}
          <option value={l}>{$t(`ui.language.${l}`)}</option>
        {/each}
      </select>
    </label>
    <label>
      {$t('ui.appearance.theme')}
      <select
        value={$theme}
        onchange={(e) => void setThemeChoice(e.currentTarget.value as Theme)}
        data-testid="theme-select"
      >
        {#each themes as th (th)}
          <option value={th}>{$t(`ui.theme.${th}`)}</option>
        {/each}
      </select>
    </label>
  </div>
</section>

<style>
  .fields {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 16px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    color: var(--muted);
    font-size: var(--fs-small);
  }
</style>
