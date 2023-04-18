import { scrapeContent } from "../../scraper/pornhub/pornhubSearchController";
import c from "../../utils/options";
import { logger } from "../../utils/logger";
import { maybeError, spacer } from "../../utils/modifier";
import { Request, Response } from "express";
const sorting = ["mr", "mv", "tr", "lg"];

export async function searchPornhub(req: Request, res: Response) {
  try {
    /**
     * @api {get} /pornhub/search Search pornhub videos
     * @apiName Search pornhub
     * @apiGroup pornhub
     * @apiDescription Search pornhub videos
     * @apiParam {String} key Keyword to search
     * @apiParam {Number} [page=1] Page number
     * @apiParam {String} [sort=mr] Sort by 
     * 
     * @apiSuccessExample {json} Success-Response:
     *    HTTP/1.1 200 OK
     *    HTTP/1.1 400 Bad Request
     * 
     * @apiExample {curl} curl
     * curl -i https://lust.scathach.id/pornhub/search?key=milf
     * curl -i https://lust.scathach.id/pornhub/search?key=milf&page=2&sort=mr
     * 
     * @apiExample {js} JS/TS
     * import axios from "axios"
     * 
     * axios.get("https://lust.scathach.id/pornhub/search?key=milf")
     * .then(res => console.log(res.data))
     * .catch(err => console.error(err))
     * 
     * @apiExample {python} Python
     * import aiohttp
     * async with aiohttp.ClientSession() as session:
     *  async with session.get("https://lust.scathach.id/pornhub/search?key=milf") as resp:
     *    print(await resp.json())
     */

    const key = req.query.key as string;
    const page = req.query.page || 1;
    const sort = req.query.sort as string;
    if (!key) throw Error("Parameter key is required");
    if (isNaN(Number(page))) throw Error("Parameter page must be a number");
    
    let url;
    if (!sort) url = `${c.PORNHUB}/video/search?search=${spacer(key)}`;
    else if (!sorting.includes(sort)) url = `${c.PORNHUB}/video/search?search=${spacer(key)}&page=${page}`;

    else url = `${c.PORNHUB}/video/search?search=${spacer(key)}&o=${sort}&page=${page}`;
    console.log(url);
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