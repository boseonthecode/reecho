// D-Bus client for talking to the Reecho service.
// Uses GNOME's built-in Gio D-Bus bindings.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {DBUS_NAME, DBUS_PATH, DBUS_INTERFACE} from './constants.js';

const ReechoServiceProxy = Gio.DBusProxy.makeProxyWrapper(
    `<node>
        <interface name="${DBUS_INTERFACE}">
            <method name="GetState">
                <arg name="state" type="s" direction="out"/>
            </method>
            <method name="GetWarning">
                <arg name="message" type="s" direction="out"/>
            </method>
            <method name="Activate">
                <arg name="ssid" type="s" direction="in"/>
                <arg name="password" type="s" direction="in"/>
                <arg name="band" type="s" direction="in"/>
            </method>
            <method name="Deactivate"/>
            <method name="GetDevices">
                <arg name="devices" type="a(ssssddd)" direction="out"/>
            </method>
            <method name="GetConfig">
                <arg name="ssid" type="s" direction="out"/>
                <arg name="password" type="s" direction="out"/>
                <arg name="band" type="s" direction="out"/>
            </method>
            <method name="SetConfig">
                <arg name="ssid" type="s" direction="in"/>
                <arg name="password" type="s" direction="in"/>
                <arg name="band" type="s" direction="in"/>
            </method>
            <method name="GetDataUsage">
                <arg name="rx" type="t" direction="out"/>
                <arg name="tx" type="t" direction="out"/>
                <arg name="limit" type="t" direction="out"/>
            </method>
            <method name="SetDataLimit">
                <arg name="bytes" type="t" direction="in"/>
            </method>
            <method name="BlacklistDevice">
                <arg name="mac" type="s" direction="in"/>
            </method>
            <method name="UnblacklistDevice">
                <arg name="mac" type="s" direction="in"/>
            </method>
            <signal name="StateChanged">
                <arg name="state" type="s"/>
            </signal>
            <signal name="DeviceConnected">
                <arg name="mac" type="s"/>
                <arg name="name" type="s"/>
            </signal>
            <signal name="DeviceDisconnected">
                <arg name="mac" type="s"/>
            </signal>
            <signal name="Warning">
                <arg name="message" type="s"/>
            </signal>
            <signal name="DataLimitReached"/>
        </interface>
    </node>`
);

export class ReechoClient {
    constructor() {
        this._proxy = null;
        this._signalIds = [];
        this._stateCallbacks = [];
        this._warningCallbacks = [];
        this._deviceCallbacks = [];
    }

    async connect() {
        return new Promise((resolve, reject) => {
            this._proxy = ReechoServiceProxy(
                Gio.DBus.session,
                DBUS_NAME,
                DBUS_PATH,
                (proxy, error) => {
                    if (error) {
                        reject(error);
                        return;
                    }
                    this._setupSignals();
                    resolve();
                }
            );
        });
    }

    _setupSignals() {
        this._signalIds.push(
            this._proxy.connectSignal('StateChanged', (_proxy, _name, params) => {
                const state = params.get_child_value(0).get_string()[0];
                for (const cb of this._stateCallbacks)
                    cb(state);
            })
        );

        this._signalIds.push(
            this._proxy.connectSignal('Warning', (_proxy, _name, params) => {
                const message = params.get_child_value(0).get_string()[0];
                for (const cb of this._warningCallbacks)
                    cb(message);
            })
        );

        this._signalIds.push(
            this._proxy.connectSignal('DeviceConnected', (_proxy, _name, params) => {
                const mac = params.get_child_value(0).get_string()[0];
                const name = params.get_child_value(1).get_string()[0];
                for (const cb of this._deviceCallbacks)
                    cb('connected', mac, name);
            })
        );

        this._signalIds.push(
            this._proxy.connectSignal('DeviceDisconnected', (_proxy, _name, params) => {
                const mac = params.get_child_value(0).get_string()[0];
                for (const cb of this._deviceCallbacks)
                    cb('disconnected', mac, '');
            })
        );
    }

    onStateChanged(callback) {
        this._stateCallbacks.push(callback);
    }

    onWarning(callback) {
        this._warningCallbacks.push(callback);
    }

    onDeviceChanged(callback) {
        this._deviceCallbacks.push(callback);
    }

    async getState() {
        const result = await this._proxy.call_finish(
            await this._proxy.call('GetState', null, Gio.DBusCallFlags.NONE, -1, null)
        );
        return result.get_child_value(0).get_string()[0];
    }

    async getWarning() {
        const result = await this._proxy.call_finish(
            await this._proxy.call('GetWarning', null, Gio.DBusCallFlags.NONE, -1, null)
        );
        return result.get_child_value(0).get_string()[0];
    }

    async activate(ssid, password, band) {
        const variant = new GLib.Variant('(sss)', [ssid, password, band]);
        await this._proxy.call('Activate', variant, Gio.DBusCallFlags.NONE, -1, null);
    }

    async deactivate() {
        await this._proxy.call('Deactivate', null, Gio.DBusCallFlags.NONE, -1, null);
    }

    async getDevices() {
        const result = await this._proxy.call_finish(
            await this._proxy.call('GetDevices', null, Gio.DBusCallFlags.NONE, -1, null)
        );
        const array = result.get_child_value(0);
        const devices = [];
        for (let i = 0; i < array.n_children(); i++) {
            const child = array.get_child_value(i);
            devices.push({
                mac: child.get_child_value(0).get_string()[0],
                ip: child.get_child_value(1).get_string()[0],
                name: child.get_child_value(2).get_string()[0],
                connectedAt: child.get_child_value(3).get_string()[0],
                bytesRx: child.get_child_value(4).get_double(),
                bytesTx: child.get_child_value(5).get_double(),
                rateRx: child.get_child_value(6).get_double(),
            });
        }
        return devices;
    }

    async getConfig() {
        const result = await this._proxy.call_finish(
            await this._proxy.call('GetConfig', null, Gio.DBusCallFlags.NONE, -1, null)
        );
        return {
            ssid: result.get_child_value(0).get_string()[0],
            password: result.get_child_value(1).get_string()[0],
            band: result.get_child_value(2).get_string()[0],
        };
    }

    async setConfig(ssid, password, band) {
        const variant = new GLib.Variant('(sss)', [ssid, password, band]);
        await this._proxy.call('SetConfig', variant, Gio.DBusCallFlags.NONE, -1, null);
    }

    async getDataUsage() {
        const result = await this._proxy.call_finish(
            await this._proxy.call('GetDataUsage', null, Gio.DBusCallFlags.NONE, -1, null)
        );
        return {
            rx: result.get_child_value(0).get_uint64(),
            tx: result.get_child_value(1).get_uint64(),
            limit: result.get_child_value(2).get_uint64(),
        };
    }

    async setDataLimit(bytes) {
        const variant = new GLib.Variant('(t)', [bytes]);
        await this._proxy.call('SetDataLimit', variant, Gio.DBusCallFlags.NONE, -1, null);
    }

    async blacklistDevice(mac) {
        const variant = new GLib.Variant('(s)', [mac]);
        await this._proxy.call('BlacklistDevice', variant, Gio.DBusCallFlags.NONE, -1, null);
    }

    async unblacklistDevice(mac) {
        const variant = new GLib.Variant('(s)', [mac]);
        await this._proxy.call('UnblacklistDevice', variant, Gio.DBusCallFlags.NONE, -1, null);
    }

    destroy() {
        for (const id of this._signalIds)
            this._proxy.disconnectSignal(id);
        this._signalIds = [];
        this._stateCallbacks = [];
        this._warningCallbacks = [];
        this._deviceCallbacks = [];
    }
}
