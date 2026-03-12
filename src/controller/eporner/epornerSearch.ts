import { scrapeContent } from "../../scraper/eporner/epornerSearchController";
import c from "../../utils/options";
import { logger } from "../../utils/logger";
import { maybeError, spacer } from "../../utils/modifier";
import { Request, Response } from "express";

export async function searchEporner(req: Request, res: Response) {
  try {
    /**
     * @api {get} /eporner/search Search eporner videos
     * @apiName Search eporner
     * @apiGroup eporner
     * @apiDescription Search eporner videos
     * @apiParam {String} key Keyword to search
     * @apiParam {Number} [page=1] Page number
     * 
     * @apiSuccessExample {json} Success-Response:
     *    HTTP/1.1 200 OK
     *    HTTP/1.1 400 Bad Request
     * 
     * @apiExample {curl} curl
     * curl -i https://lust.scathach.id/eporner/search?key=milf
     * curl -i https://lust.scathach.id/eporner/search?key=milf&page=2
     * 
     * @apiExample {js} JS/TS
     * import axios from "axios"
     * 
     * axios.get("https://lust.scathach.id/eporner/search?key=milf")
     * .then(res => console.log(res.data))
     * .catch(err => console.error(err))
     * 
     * @apiExample {python} Python
     * import aiohttp
     * async with aiohttp.ClientSession() as session:
     *  async with session.get("https://lust.scathach.id/eporner/search?key=milf") as resp:
     *    print(await resp.json())
     */

    // https://www.eporner.com/tag/milf/

    const key = req.query.key as string;
    const page = Number(req.query.page || 1);

    if (!key) throw Error("Parameter key is required");
    if (isNaN(page)) throw Error("Parameter page must be a number");

    const slug = key
        .toLowerCase()
        .trim()
        .replace(/\s+/g, "-");

    const url =
        page === 1
        ? `${c.EPORNER}/tag/${slug}/`
        : `${c.EPORNER}/tag/${slug}/${page}/`;

    const data = await scrapeContent(url);

    logger.info({
        path: req.path,
        query: req.query,
        method: req.method,
        ip: req.ip,
        useragent: req.get("User-Agent")
    });

    return res.json(data);
    } catch (err) {
    const e = err as Error;
    res.status(400).json(maybeError(false, e.message));
    }
}