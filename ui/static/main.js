import init, { run_app } from '../pkg/netholdem_ui.js';

async function main() {
    await init('/netholdem_ui_bg.wasm');
    run_app();
}
main()