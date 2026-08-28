// UI components for the GNOME Shell extension.
// Provides a toggle button for Quick Settings and status display.

import * as QuickSettings from 'resource:///org/gnome/shell/ui/quickSettings.js';

import {ReechoClient} from './dbusClient.js';
import {STATE_ACTIVE, STATE_ACTIVATING, STATE_INACTIVE} from './constants.js';

export class ReechoToggle {
    constructor(extension) {
        this._extension = extension;
        this._client = new ReechoClient();
        this._toggle = null;
        this._statusItem = null;
        this._panelMenuButton = null;
    }

    async enable() {
        try {
            await this._client.connect();
        } catch (e) {
            log(`Reecho: failed to connect to D-Bus service: ${e.message}`);
            return;
        }

        // Create the toggle button for Quick Settings.
        this._toggle = new QuickSettings.ToggleButton({
            title: 'Reecho',
            subtitle: 'Hotspot inactive',
            iconName: 'network-wireless-disabled-symbolic',
            toggleMode: true,
        });

        this._toggle.connect('clicked', () => {
            this._onToggleClicked();
        });

        // Create a status indicator in the quick settings menu.
        this._statusItem = new QuickSettings.QuickMenuToggle({
            title: 'Reecho Hotspot',
            subtitle: 'Inactive',
            iconName: 'network-wireless-disabled-symbolic',
            toggleMode: true,
        });

        this._statusItem.connect('clicked', () => {
            this._onToggleClicked();
        });

        // Add to quick settings.
        this._extension._quickSettingsMenu.addExternalIndicator(this._statusItem);

        // Listen for state changes from the service.
        this._client.onStateChanged(state => this._updateUI(state));
        this._client.onWarning(message => this._showWarning(message));

        // Get initial state.
        try {
            const state = await this._client.getState();
            this._updateUI(state);
        } catch (e) {
            log(`Reecho: failed to get initial state: ${e.message}`);
        }
    }

    disable() {
        if (this._toggle) {
            this._toggle.destroy();
            this._toggle = null;
        }
        if (this._statusItem) {
            this._statusItem.destroy();
            this._statusItem = null;
        }
        if (this._client) {
            this._client.destroy();
            this._client = null;
        }
    }

    async _onToggleClicked() {
        try {
            const state = await this._client.getState();
            if (state === STATE_ACTIVE) {
                await this._client.deactivate();
            } else if (state === STATE_INACTIVE) {
                const config = await this._client.getConfig();
                await this._client.activate(config.ssid, config.password, config.band);
                this._updateUI(STATE_ACTIVATING);
            }
        } catch (e) {
            log(`Reecho: toggle failed: ${e.message}`);
            this._updateUI(STATE_INACTIVE);
        }
    }

    _updateUI(state) {
        const active = state === STATE_ACTIVE;
        const activating = state === STATE_ACTIVATING;

        if (this._toggle) {
            this._toggle.set({checked: active});
            this._toggle.set({
                subtitle: active ? 'Hotspot active' :
                    activating ? 'Activating...' : 'Hotspot inactive',
                iconName: active ?
                    'network-wireless-signal-excellent-symbolic' :
                    'network-wireless-disabled-symbolic',
            });
        }

        if (this._statusItem) {
            this._statusItem.set({checked: active});
            this._statusItem.set({
                subtitle: active ? 'Hotspot active' :
                    activating ? 'Activating...' : 'Inactive',
                iconName: active ?
                    'network-wireless-signal-excellent-symbolic' :
                    'network-wireless-disabled-symbolic',
            });
        }

        // Update subtitle with device count when active.
        if (active) {
            this._updateDeviceCount();
        }
    }

    async _updateDeviceCount() {
        try {
            const devices = await this._client.getDevices();
            const count = devices.length;
            const subtitle = `${count} device${count !== 1 ? 's' : ''} connected`;
            if (this._toggle)
                this._toggle.set({subtitle});
            if (this._statusItem)
                this._statusItem.set({subtitle});
        } catch (e) {
            // Ignore errors during device count update.
        }
    }

    _showWarning(message) {
        if (this._toggle) {
            this._toggle.set({subtitle: `Warning: ${message}`});
        }
    }
}
