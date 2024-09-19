// Prelude script for every `function` node
const RED = (function () {
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
                    var m = __edgelink.deepClone(msg);
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
