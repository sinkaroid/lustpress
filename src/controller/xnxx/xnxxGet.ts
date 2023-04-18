import { scrapeContent } from "../../scraper/xnxx/xnxxGetController";
import c from "../../utils/options";
import { logger } from "../../utils/logger";
import { maybeError } from "../../utils/modifier";
import { Request, Response } from "express";

export async function getXnxx(req: Request, res: Response) {
  try {
    const id = req.query.id as string;
    if (!id) throw Error("Parameter id is required");

    /**
     * @api {get} /xnxx/get?id=:id Get xnxx
     * @apiName Get xnxx
     * @apiGroup xnxx
     * @apiDescription Get a xnxx video based on id
     * 
     * @apiParam {String} id Video ID
     * 
     * @apiSuccessExample {json} Success-Response:
     *   HTTP/1.1 200 OK
     *   HTTP/1.1 400 Bad Request
     * 
     * @apiExample {curl} curl
     * curl -i https://lust.scathach.id/xnxx/get?id=video-17vah71a/makima_y_denji
     * 
     * @apiExample {js} JS/TS
     * import axios from "axios"
     * 
     * axios.get("https://lust.scathach.id/xnxx/get?id=video-17vah71a/makima_y_denji")
     * .then(res => console.log(res.data))
     * .catch(err => console.error(err))
     * 
     * @apiExample {python} Python
     * import aiohttp
     * async with aiohttp.ClientSession() as session:
     *  async with session.get("https://lust.scathach.id/xnxx/get?id=video-17vah71a/makima_y_denji") as resp:
     *    print(await resp.json())
     */

    const url = `${c.XNXX}/${id}`;
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
