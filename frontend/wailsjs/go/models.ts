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
	    }
	}

}

