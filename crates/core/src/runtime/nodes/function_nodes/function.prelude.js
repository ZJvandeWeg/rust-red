// Prelude script for every `function` node

const RED = (function () {
    function __el_deepClone(obj) {
        if (obj === null || typeof obj !== 'object') {
            return obj;
        }

        if (obj instanceof Date) {
            return new Date(obj.getTime());
        }

        if (Array.isArray(obj)) {
            const arrCopy = [];
            for (let i = 0; i < obj.length; i++) {
                arrCopy[i] = __el_deepClone(obj[i]);
            }
            return arrCopy;
        }

        const objCopy = {};
        for (const key in obj) {
            if (obj.hasOwnProperty(key)) {
                objCopy[key] = __el_deepClone(obj[key]);
            }
        }
        return objCopy;
    }

    return {
        util: {
            cloneMessage: function (msg) {
                // FROM node-red
                if (typeof msg !== "undefined" && msg !== null) {
                    // Temporary fix for #97
                    // TODO: remove this http-node-specific fix somehow
                    var req = msg.req;
                    var res = msg.res;
                    delete msg.req;
                    delete msg.res;
                    var m = __el_deepClone(msg);
                    if (req) {
                        m.req = req;
                        msg.req = req;
                    }
                    if (res) {
                        m.res = res;
                        msg.res = res;
                    }
                    return m;
                }
                return msg;

            }
        }
    };
})();
