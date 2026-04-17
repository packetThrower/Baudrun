export namespace main {
	
	export class ControlLines {
	    dtr: boolean;
	    rts: boolean;
	
	    static createFrom(source: any = {}) {
	        return new ControlLines(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.dtr = source["dtr"];
	        this.rts = source["rts"];
	    }
	}

}

export namespace profiles {
	
	export class Profile {
	    id: string;
	    name: string;
	    portName: string;
	    baudRate: number;
	    dataBits: number;
	    parity: string;
	    stopBits: string;
	    flowControl: string;
	    lineEnding: string;
	    localEcho: boolean;
	    highlight: boolean;
	    themeId: string;
	    dtrOnConnect: string;
	    rtsOnConnect: string;
	    dtrOnDisconnect: string;
	    rtsOnDisconnect: string;
	    hexView: boolean;
	    timestamps: boolean;
	    logEnabled: boolean;
	    // Go type: time
	    createdAt: any;
	    // Go type: time
	    updatedAt: any;
	
	    static createFrom(source: any = {}) {
	        return new Profile(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.portName = source["portName"];
	        this.baudRate = source["baudRate"];
	        this.dataBits = source["dataBits"];
	        this.parity = source["parity"];
	        this.stopBits = source["stopBits"];
	        this.flowControl = source["flowControl"];
	        this.lineEnding = source["lineEnding"];
	        this.localEcho = source["localEcho"];
	        this.highlight = source["highlight"];
	        this.themeId = source["themeId"];
	        this.dtrOnConnect = source["dtrOnConnect"];
	        this.rtsOnConnect = source["rtsOnConnect"];
	        this.dtrOnDisconnect = source["dtrOnDisconnect"];
	        this.rtsOnDisconnect = source["rtsOnDisconnect"];
	        this.hexView = source["hexView"];
	        this.timestamps = source["timestamps"];
	        this.logEnabled = source["logEnabled"];
	        this.createdAt = this.convertValues(source["createdAt"], null);
	        this.updatedAt = this.convertValues(source["updatedAt"], null);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}

}

export namespace serial {
	
	export class PortInfo {
	    name: string;
	    isUSB: boolean;
	    vid?: string;
	    pid?: string;
	    serialNumber?: string;
	    product?: string;
	    chipset?: string;
	
	    static createFrom(source: any = {}) {
	        return new PortInfo(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.name = source["name"];
	        this.isUSB = source["isUSB"];
	        this.vid = source["vid"];
	        this.pid = source["pid"];
	        this.serialNumber = source["serialNumber"];
	        this.product = source["product"];
	        this.chipset = source["chipset"];
	    }
	}
	export class USBSerialCandidate {
	    vid: string;
	    pid: string;
	    chipset: string;
	    manufacturer?: string;
	    product?: string;
	    serialNumber?: string;
	    driverURL?: string;
	    reason?: string;
	
	    static createFrom(source: any = {}) {
	        return new USBSerialCandidate(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.vid = source["vid"];
	        this.pid = source["pid"];
	        this.chipset = source["chipset"];
	        this.manufacturer = source["manufacturer"];
	        this.product = source["product"];
	        this.serialNumber = source["serialNumber"];
	        this.driverURL = source["driverURL"];
	        this.reason = source["reason"];
	    }
	}

}

export namespace settings {
	
	export class Settings {
	    defaultThemeId: string;
	    fontSize?: number;
	    logDir?: string;
	
	    static createFrom(source: any = {}) {
	        return new Settings(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.defaultThemeId = source["defaultThemeId"];
	        this.fontSize = source["fontSize"];
	        this.logDir = source["logDir"];
	    }
	}

}

export namespace themes {
	
	export class Theme {
	    id: string;
	    name: string;
	    source: string;
	    background: string;
	    foreground: string;
	    cursor: string;
	    cursorAccent?: string;
	    selection: string;
	    selectionForeground?: string;
	    black: string;
	    red: string;
	    green: string;
	    yellow: string;
	    blue: string;
	    magenta: string;
	    cyan: string;
	    white: string;
	    brightBlack: string;
	    brightRed: string;
	    brightGreen: string;
	    brightYellow: string;
	    brightBlue: string;
	    brightMagenta: string;
	    brightCyan: string;
	    brightWhite: string;
	
	    static createFrom(source: any = {}) {
	        return new Theme(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.source = source["source"];
	        this.background = source["background"];
	        this.foreground = source["foreground"];
	        this.cursor = source["cursor"];
	        this.cursorAccent = source["cursorAccent"];
	        this.selection = source["selection"];
	        this.selectionForeground = source["selectionForeground"];
	        this.black = source["black"];
	        this.red = source["red"];
	        this.green = source["green"];
	        this.yellow = source["yellow"];
	        this.blue = source["blue"];
	        this.magenta = source["magenta"];
	        this.cyan = source["cyan"];
	        this.white = source["white"];
	        this.brightBlack = source["brightBlack"];
	        this.brightRed = source["brightRed"];
	        this.brightGreen = source["brightGreen"];
	        this.brightYellow = source["brightYellow"];
	        this.brightBlue = source["brightBlue"];
	        this.brightMagenta = source["brightMagenta"];
	        this.brightCyan = source["brightCyan"];
	        this.brightWhite = source["brightWhite"];
	    }
	}

}

