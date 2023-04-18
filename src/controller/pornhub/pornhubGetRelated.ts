import { scrapeContent } from "../../scraper/pornhub/pornhubSearchController";
import c from "../../utils/options";
import { logger } from "../../utils/logger";
import { maybeError } from "../../utils/modifier";
import { Request, Response } from "express";

export async function relatedPornhub(req: Request, res: Response) {
  try {
    const id = req.query.id as string;
    if (!id) throw Error("Parameter id is required");

    /**
     * @api {get} /pornhub/get?id=:id Get Pornhub related videos
     * @apiName Get pornhub related videos
     * @apiGroup pornhub
     * @apiDescription Get a related pornhub videos based on id
     * 
     * @apiParam {String} id Video ID
     * 
     * @apiSuccessExample {json} Success-Response:
     *   HTTP/1.1 200 OK
     *   HTTP/1.1 400 Bad Request
     * 
     * @apiExample {curl} curl
     * curl -i https://lust.scathach.id/pornhub/get?id=ph63c4e1dc48fe7
     * 
     * @apiExample {js} JS/TS
     * import axios from "axios"
     * 
     * axios.get("https://lust.scathach.id/pornhub/get?id=ph63c4e1dc48fe7")
     * .then(res => console.log(res.data))
     * .catch(err => console.error(err))
     * 
     * @apiExample {python} Python
     * import aiohttp
     * async with aiohttp.ClientSession() as session:
     *  async with session.get("https://lust.scathach.id/pornhub/get?id=ph63c4e1dc48fe7") as resp:
     *    print(await resp.json())
     */

    const url = `${c.PORNHUB}/view_video.php?viewkey=${id}`;
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
