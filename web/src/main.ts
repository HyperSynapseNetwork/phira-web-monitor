/**
 * Phira Web Monitor - Demo Entry Point
 * 
 * This file demonstrates loading and using the WASM module.
 */

import init, { greet } from '../../monitor/pkg/phira_web_monitor.js';

async function main() {
  const resultEl = document.getElementById('result');
  if (!resultEl) return;

  try {
    // Initialize WASM module
    await init();
    
    // Test the greet function
    const message = greet('Phira Player');
    
    resultEl.textContent = message;
    resultEl.className = '';
  } catch (error) {
    resultEl.textContent = `Error: ${error}`;
    resultEl.className = 'error';
    console.error('Failed to load WASM:', error);
  }
}

main();
