/**
 * Phira Web Monitor - Demo Entry Point
 * 
 * This file demonstrates loading and using the WASM module,
 * with server-side chart parsing via the proxy.
 */

import init, { greet, decode_chart } from '../../monitor-client/pkg/monitor_client.js';

// The proxy server handles /chart/:id requests, parses them, 
// and returns bincode-encoded binary data.
const PROXY_CHART_API = '/chart'; 

async function main() {
  const resultEl = document.getElementById('result');
  const parseBtn = document.getElementById('parse-btn') as HTMLButtonElement;
  const chartIdInput = document.getElementById('chart-id') as HTMLInputElement;
  const parseResultEl = document.getElementById('parse-result');
  const statsEl = document.getElementById('stats');
  
  if (!resultEl || !parseBtn || !chartIdInput || !parseResultEl || !statsEl) {
    console.error('Missing DOM elements');
    return;
  }

  try {
    // Initialize WASM module
    await init();
    
    // Test the greet function
    const message = greet('Phira Player');
    resultEl.textContent = message;
    resultEl.className = '';
    
    // Enable parse button
    parseBtn.disabled = false;
    
    // Setup parse button click handler
    parseBtn.addEventListener('click', async () => {
      const chartId = chartIdInput.value.trim();
      if (!chartId) {
        parseResultEl.textContent = 'Please enter a chart ID';
        parseResultEl.className = 'error';
        return;
      }
      
      await fetchAndDecodeChart(chartId, parseBtn, parseResultEl, statsEl);
    });
    
    // Allow Enter key to trigger parse
    chartIdInput.addEventListener('keypress', (e) => {
      if (e.key === 'Enter' && !parseBtn.disabled) {
        parseBtn.click();
      }
    });
    
  } catch (error) {
    resultEl.textContent = `Error: ${error}`;
    resultEl.className = 'error';
    console.error('Failed to load WASM:', error);
  }
}

async function fetchAndDecodeChart(
  chartId: string, 
  button: HTMLButtonElement, 
  resultEl: HTMLElement,
  statsEl: HTMLElement
) {
  button.disabled = true;
  resultEl.className = 'loading';
  resultEl.textContent = `Processing chart ${chartId} on server...`;
  statsEl.style.display = 'none';
  
  try {
    // Step 1: Fetch processed chart data from proxy (binary)
    console.time('fetch');
    const response = await fetch(`${PROXY_CHART_API}/${chartId}`);
    if (!response.ok) {
      const text = await response.text();
      throw new Error(`Server error: ${response.status} - ${text}`);
    }
    
    const buffer = await response.arrayBuffer();
    const bytes = new Uint8Array(buffer);
    console.timeEnd('fetch');
    
    console.log(`Received ${bytes.length} bytes of chart data`);
    resultEl.textContent = `Decoding ${bytes.length} bytes...`;
    
    // Step 2: Decode using WASM (bincode -> struct)
    console.time('decode');
    const resultJson = decode_chart(bytes);
    console.timeEnd('decode');
    
    const parsed = JSON.parse(resultJson);
    
    if (parsed.success) {
      resultEl.className = 'success';
      resultEl.textContent = `✓ Chart loaded successfully!\n\n(Server-side parsed & transferred as binary)`;
      
      // Update stats
      statsEl.style.display = 'grid';
      (document.getElementById('stat-lines') as HTMLElement).textContent = parsed.lineCount.toString();
      (document.getElementById('stat-notes') as HTMLElement).textContent = parsed.noteCount.toString();
      (document.getElementById('stat-offset') as HTMLElement).textContent = (parsed.offset * 1000).toFixed(0);
      
      console.log('Chart loaded:', parsed);
    } else {
      throw new Error(parsed.error || 'Unknown decode error');
    }
    
  } catch (error) {
    resultEl.className = 'error';
    resultEl.textContent = `Error: ${error}`;
    console.error('Load error:', error);
    statsEl.style.display = 'none';
  } finally {
    button.disabled = false;
  }
}

main();
