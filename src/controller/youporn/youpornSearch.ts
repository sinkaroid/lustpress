import { scrapeContent } from "../../scraper/youporn/youpornSearchController";
import c from "../../utils/options";
import { logger } from "../../utils/logger";
import { maybeError, spacer } from "../../utils/modifier";
import { Request, Response } from "express";

export async function searchYouporn(req: Request, res: Response) {
  try {
    /**
     * @api {get} /youporn/search Search youporn videos
     * @apiName Search youporn
     * @apiGroup youporn
     * @apiDescription Search youporn videos
     * @apiParam {String} key Keyword to search
     * @apiParam {Number} [page=1] Page number
     * 
     * @apiSuccessExample {json} Success-Response:
     *    HTTP/1.1 200 OK
     *    HTTP/1.1 400 Bad Request
     * 
     * @apiExample {curl} curl
     * curl -i https://lust.scathach.id/youporn/search?key=milf
     * curl -i https://lust.scathach.id/youporn/search?key=milf&page=2
     * 
     * @apiExample {js} JS/TS
     * import axios from "axios"
     * 
     * axios.get("https://lust.scathach.id/youporn/search?key=milf")
     * .then(res => console.log(res.data))
     * .catch(err => console.error(err))
     * 
     * @apiExample {python} Python
     * import aiohttp
     * async with aiohttp.ClientSession() as session:
     *  async with session.get("https://lust.scathach.id/youporn/search?key=milf") as resp:
     *    print(await resp.json())
     */

    const key = req.query.key as string;
    const page = req.query.page || 1;
    if (!key) throw Error("Parameter key is required");
    if (isNaN(Number(page))) throw Error("Parameter page must be a number");

    const url = `${c.YOUPORN}/search/?query=${spacer(key)}&page=${page}`;
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