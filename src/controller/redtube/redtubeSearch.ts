import { scrapeContent } from "../../scraper/redtube/redtubeSearchController";
import c from "../../utils/options";
import { logger } from "../../utils/logger";
import { maybeError, spacer } from "../../utils/modifier";
import { Request, Response } from "express";

export async function searchRedtube(req: Request, res: Response) {
  try {
    /**
     * @api {get} /redtube/search Search redtube videos
     * @apiName Search redtube
     * @apiGroup redtube
     * @apiDescription Search redtube videos
     * @apiParam {String} key Keyword to search
     * @apiParam {Number} [page=1] Page number
     * 
     * @apiSuccessExample {json} Success-Response:
     *    HTTP/1.1 200 OK
     *    HTTP/1.1 400 Bad Request
     * 
     * @apiExample {curl} curl
     * curl -i https://lust.scathach.id/redtube/search?key=milf
     * curl -i https://lust.scathach.id/redtube/search?key=milf&page=2
     * 
     * @apiExample {js} JS/TS
     * import axios from "axios"
     * 
     * axios.get("https://lust.scathach.id/redtube/search?key=milf")
     * .then(res => console.log(res.data))
     * .catch(err => console.error(err))
     * 
     * @apiExample {python} Python
     * import aiohttp
     * async with aiohttp.ClientSession() as session:
     *  async with session.get("https://lust.scathach.id/redtube/search?key=milf") as resp:
     *    print(await resp.json())
     */

    const key = req.query.key as string;
    const page = req.query.page || 1;
    if (!key) throw Error("Parameter key is required");
    if (isNaN(Number(page))) throw Error("Parameter page must be a number");

    const url = `${c.REDTUBE}/?search=${spacer(key)}&page=${page}`;
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