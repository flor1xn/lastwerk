<script lang="ts">
  import { t } from '../i18n';
</script>

<span class="estimated" title={$t('ui.estimated_hint')} data-testid="estimated"
  >{$t('ui.estimated')}</span
>

<style>
  .estimated {
    display: inline-block;
    margin-left: 4px;
    padding: 0 4px;
    border: 1px solid var(--warn);
    border-radius: 3px;
    color: var(--warn);
    font-size: var(--fs-label);
    line-height: 14px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    vertical-align: 1px;
  }
</style>
