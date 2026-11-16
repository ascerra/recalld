#!/usr/bin/env python3
"""Actual RPC-boundary and supervisor trace tests; injected faults are unit fixtures."""
import copy
import io
import json
import os
from pathlib import Path
import runpy
import sys
import tempfile
import unittest

import test_public_mcp_evaluation as baseline
m = baseline.m

a = runpy.run_path(str(Path(__file__).with_name('public-mcp-actor.py')))


class Exchange(unittest.TestCase):
    def exchange(self, data=b'{"id":1,"result":{"ok":true}}\n', mode='full', notification=False, open_writer=False, limit=1024):
        r,w=os.pipe();read=os.fdopen(r,'rb',buffering=0);self.addCleanup(read.close)
        if data: os.write(w,data)
        if open_writer:self.addCleanup(os.close,w)
        else:os.close(w)
        class Writer:
            def write(self, raw):
                self.raw=raw
                if mode=='error': raise OSError('injected write failure')
                return 2 if mode=='partial' else len(raw)
        writer=Writer();request=dict(jsonrpc='2.0',method='fixture',params={'text':'\u03bb'})
        if not notification:request['id']=1
        events=[]
        def emit(event,response,accounting):events.append(dict(event=event,response=response,accounting=accounting,request=request,stepId='1',deliveryOrdinal=0,tag='unit'))
        error=None
        try:a['accounted_exchange'](read,writer,request,notification,emit,timeout=.02,limit=limit)
        except BaseException as caught:error=caught
        return request,events,error

    def test_exact_buffers_and_notification_not_inferred_tokens(self):
        request,events,error=self.exchange();self.assertIsNone(error)
        self.assertEqual([e['event'] for e in events],['attempt','outcome'])
        facts=events[-1]['accounting'];outgoing=bytes.fromhex(facts['requestWireHex']);incoming=bytes.fromhex(facts['responseWireHex'])
        self.assertEqual(json.loads(outgoing),request);self.assertEqual(facts['requestBytesWritten'],str(len(outgoing)))
        self.assertEqual(facts['responseBytesRead'],str(len(incoming)));self.assertEqual(facts['category'],'success')
        self.assertTrue(facts['observedClientElapsedNs'].isdecimal())
        _,notification,error=self.exchange(data=b'',notification=True);self.assertIsNone(error)
        self.assertEqual(notification[-1]['accounting']['category'],'notification')
        self.assertEqual(notification[-1]['accounting']['responseBytesRead'],'0')

    def test_partial_write_error_eof_parse_id_timeout_and_oversize_remain_unknown(self):
        cases=[({'mode':'partial'},'write_error','2'),({'mode':'error'},'write_error',None),
               ({'data':b'{"id":1'},'eof','full'),({'data':b'bad\n'},'response_parse','full'),
               ({'data':b'{"id":2,"result":{}}\n'},'response_id','full'),
               ({'data':b'{"id":1}\n'},'response_shape','full'),
               ({'data':b'', 'open_writer':True},'timeout','full'),
               ({'data':b'x'*40, 'limit':32},'oversized_response','full')]
        for kwargs,category,written in cases:
            with self.subTest(category=category,kwargs=kwargs):
                _,events,error=self.exchange(**kwargs);self.assertIsNotNone(error)
                self.assertEqual([e['event'] for e in events],['attempt','error'])
                fact=events[-1]['accounting'];self.assertEqual(fact['category'],category);self.assertEqual(fact['mutationOutcome'],'unknown')
                self.assertEqual(fact['requestBytesWritten'],fact['requestBytesIntended'] if written=='full' else written)
                self.assertEqual(fact['responseBytesRead'],str(len(bytes.fromhex(fact['responseWireHex']))))

    def test_protocol_and_tool_errors_are_observed_responses(self):
        for raw,category in ((b'{"id":1,"error":{"code":-32601}}\n','protocol_error'),(b'{"id":1,"result":{"isError":true}}\n','tool_error')):
            _,events,error=self.exchange(raw);self.assertIsNone(error);self.assertEqual(events[-1]['accounting']['category'],category)


class Trace(unittest.TestCase):
    exchange = Exchange.exchange
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup);self.root=Path(self.tmp.name)

    def test_attempt_flushed_before_consuming_outcome_and_exact_reconciliation(self):
        _,events,error=self.exchange();self.assertIsNone(error)
        path=self.root/'trace.jsonl';trace=m['TraceRecorder'](path,'a'*64,'unit')
        trace.consume(events[0]);self.assertEqual(m['rows'](path)[0]['eventKind'],'attempt')
        self.assertEqual(len(m['rows'](path)),1)
        trace.consume(events[1]);trace.close()
        rows=m['rows'](path);summary=m['reconcile_accounting'](rows,[])
        self.assertEqual((summary['attemptedActions'],summary['terminalRpcObservations'],summary['incompleteActions']),('1','1','0'))
        self.assertIsNone(summary['measuredTokens']);self.assertIsNone(summary['productionCost']);self.assertIsNone(summary['httpFramingBytes'])
        self.assertEqual(summary['requestBytesWrittenKnown'],events[-1]['accounting']['requestBytesWritten'])
        changed=copy.deepcopy(rows);changed[1]['accounting']['responseBytesRead']='999'
        with self.assertRaisesRegex(ValueError,'buffer byte'):m['reconcile_accounting'](changed,[])
        with self.assertRaises(ValueError):m['reconcile_accounting'](rows[:-1],[])
        with self.assertRaises(ValueError):m['reconcile_accounting'](rows+[rows[1]],[])

    def test_actor_exit_retains_incomplete_attempt_without_rollback(self):
        _,events,_=self.exchange();trace=m['TraceRecorder'](self.root/'trace.jsonl','a'*64,'unit')
        code='import sys;sys.stdin.read();print('+repr(json.dumps(events[0]))+',flush=True)'
        with (self.root/'wire').open('wb') as wire,(self.root/'stderr').open('wb') as err:
            result=m['run_accounted_actor']([sys.executable,'-c',code],{},wire,err,self.root,dict(PATH='/usr/bin:/bin'),(),trace)
        trace.close();self.assertEqual(result.returncode,1)
        rows=m['rows'](self.root/'trace.jsonl');self.assertEqual([r['eventKind'] for r in rows],['attempt','process_error','incomplete'])
        value=m['reconcile_accounting'](rows,[]);self.assertEqual(value['incompleteActions'],'1');self.assertEqual(value['requestWriteUnknownActions'],'1')
        self.assertEqual(value['terminalRpcObservations'],'0');self.assertIsNone(rows[-1]['outcome'])
        self.assertEqual(rows[-1]['accounting']['mutationOutcome'],'unknown')

    def test_actor_timeout_retains_partial_evidence_and_joins_owned_child(self):
        _,events,_=self.exchange();trace=m['TraceRecorder'](self.root/'trace.jsonl','a'*64,'unit')
        code='import sys,time;sys.stdin.read();print('+repr(json.dumps(events[0]))+',flush=True);time.sleep(5)'
        with (self.root/'wire').open('wb') as wire,(self.root/'stderr').open('wb') as err:
            with self.assertRaises(TimeoutError):m['run_accounted_actor']([sys.executable,'-c',code],{},wire,err,self.root,dict(PATH='/usr/bin:/bin'),(),trace,timeout=.2)
        trace.close();rows=m['rows'](self.root/'trace.jsonl');self.assertEqual(rows[-1]['eventKind'],'incomplete')
        self.assertEqual(m['reconcile_accounting'](rows,[])['incompleteActions'],'1')


class AccountingPreflight(baseline.Preflight):
    def test_accounting_policy_and_old_track_boundary(self):
        original=copy.deepcopy(self.manifest);self.manifest.update(track=m['ACCOUNTING_TRACK'],accountingPolicy=m['ACCOUNTING_POLICY'])
        self.assertEqual(self.validate()[0]['accountingPolicy'],m['ACCOUNTING_POLICY'])
        good=copy.deepcopy(self.manifest)
        for key,value in (('accountingVersion',True),('accountingVersion',2),('tokenizer','invented'),('answerContextCapTokens',4096),('productionPricing',0),('sampled',True)):
            altered=copy.deepcopy(good);altered['accountingPolicy'][key]=value
            with self.assertRaisesRegex(ValueError,'unsupported accounting'):self.validate(altered)
        altered=copy.deepcopy(good);altered['track']=m['TRACK']
        with self.assertRaises(ValueError):self.validate(altered)
        self.manifest=original;self.assertEqual(self.validate()[0]['track'],m['TRACK'])


if __name__=='__main__':unittest.main()
