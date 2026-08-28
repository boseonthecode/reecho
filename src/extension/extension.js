// Reecho GNOME Shell extension entry point.
// Registers the toggle button in Quick Settings.

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

import {ReechoToggle} from './ui.js';

export default class ReechoExtension extends Extension {
    constructor(metadata) {
        super(metadata);
        this._toggle = null;
    }

    enable() {
        this._toggle = new ReechoToggle(this);
        this._toggle.enable();
    }

    disable() {
        if (this._toggle) {
            this._toggle.disable();
            this._toggle = null;
        }
    }
}
