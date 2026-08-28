// D-Bus interface constants for the Reecho service.
// Mirrored from shared/constants.rs.

export const DBUS_NAME = 'org.reecho.Service';
export const DBUS_PATH = '/org/reecho/Service';
export const DBUS_INTERFACE = 'org.reecho.Service';

export const STATE_INACTIVE = 'inactive';
export const STATE_ACTIVATING = 'activating';
export const STATE_ACTIVE = 'active';
export const STATE_FAILED = 'failed';
