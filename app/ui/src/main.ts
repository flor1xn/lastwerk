import { mount } from 'svelte';
import './fonts.css';
import './app.css';
import App from './App.svelte';

const target = document.getElementById('app');
if (!target) {
  throw new Error('Wurzelelement #app fehlt');
}

mount(App, { target });
