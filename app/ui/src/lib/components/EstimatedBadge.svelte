<script lang="ts">
  import { t } from '../i18n';
</script>

<span class="estimated" title={$t('ui.estimated_hint')} data-testid="estimated"
  >{$t('ui.estimated')}</span
>

<style>
  /* "geschätzt" als leises Wort neben der Zahl, kursiv statt umrandet: Es ist ein Hinweis, kein
     Fehler. Die Erklärung steht im Titel. */
  .estimated {
    margin-left: 4px;
    color: var(--warn);
    font-size: var(--fs-small);
    font-style: italic;
  }
</style>
